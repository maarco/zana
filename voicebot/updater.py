"""Zana updates for Voicebot: find a newer release on GitHub, check it, and swap it in for the running app.
Standard library only (run in CI); macOS tools do the work: curl, hdiutil, ditto, codesign, spctl, osascript."""
import hashlib
import json
import os
import plistlib
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = "maarco/zana"
STATE = "update.json"  # in the Voicebot folder: the version already offered, and the update just installed


class UpdateError(Exception):
    """Why an update can't go ahead, in words Voicebot can say."""


def newer(latest: str, current: str) -> bool:
    def key(version: str) -> tuple[int, ...]:
        return tuple(int(n) for n in re.findall(r"\d+", version)[:3])
    return key(latest) > key(current)


def installed_app(code_dir: str) -> Path | None:
    """The Zana.app this code runs from (its Contents/Resources/voicebot). ZANA_APP names the app when the
    code runs from a source folder, in development."""
    code = Path(code_dir).resolve()
    app = Path(os.getenv("ZANA_APP") or (code.parents[2] if len(code.parents) > 2 else code))
    return app if app.suffix == ".app" and (app / "Contents" / "Info.plist").is_file() else None


def info(app: Path) -> dict:
    with open(app / "Contents" / "Info.plist", "rb") as f:
        return plistlib.load(f)


def run(*args: str, timeout: float = 120) -> str:
    """A command's output (stdout and stderr: codesign reports on stderr); a failure raises UpdateError."""
    try:
        out = subprocess.run(args, capture_output=True, text=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        raise UpdateError(f"{args[0]} took too long") from None
    if out.returncode:
        raise UpdateError(f"{args[0]} failed: {(out.stderr or out.stdout).strip()[:300]}")
    return out.stdout + out.stderr


def parse_release(release: dict) -> dict:
    """The version of a GitHub release and the links to its disk image and that image's checksum."""
    assets = {a["name"]: a["browser_download_url"] for a in release.get("assets") or []}
    dmg = next((name for name in assets if name.endswith(".dmg")), None)
    if not dmg or f"{dmg}.sha256" not in assets:
        raise UpdateError(f"release {release.get('tag_name')} has no disk image with a checksum")
    return {"version": str(release["tag_name"]).lstrip("vV"), "dmg": assets[dmg], "sha256": assets[f"{dmg}.sha256"]}


def check(code_dir: str) -> dict:
    """The running app, its version and the newest release; "newer" says whether that release is an update."""
    app = installed_app(code_dir)
    if app is None:
        raise UpdateError("this copy of Zana runs from source, so it doesn't update itself")
    current = version(app)
    try:
        release = json.loads(run("curl", "-fsSL", "-m", "20", "-H", "Accept: application/vnd.github+json",
                                 f"https://api.github.com/repos/{REPO}/releases/latest"))
    except (UpdateError, ValueError) as exc:
        raise UpdateError(f"GitHub didn't answer ({exc})") from None
    found = parse_release(release)
    return {"app": str(app), "current": current, **found, "newer": newer(found["version"], current)}


def team_id(app: Path) -> str:
    found = re.search(r"^TeamIdentifier=(\w+)$", run("codesign", "-dv", "--verbose=2", str(app)), re.MULTILINE)
    if not found or found.group(1) == "not":  # "TeamIdentifier=not set": an unsigned or ad-hoc build
        raise UpdateError("this copy of Zana isn't signed by its developer, so it can't vouch for an update")
    return found.group(1)


def verify(app: Path, team: str, bundle_id: str, want: str) -> None:
    """The same developer's signature (an Apple-issued certificate for the running app's team), accepted by
    Gatekeeper (notarized), and the app and version the release says."""
    run("codesign", "--verify", "--deep", "--strict", "--test-requirement",
        f'=anchor apple generic and certificate leaf[subject.OU] = "{team}"', str(app))
    run("spctl", "--assess", "--type", "execute", str(app))
    plist = info(app)
    if plist.get("CFBundleIdentifier") != bundle_id or plist.get("CFBundleShortVersionString") != want:
        raise UpdateError(f"the downloaded app is {plist.get('CFBundleIdentifier')} "
                          f"{plist.get('CFBundleShortVersionString')}, not Zana {want}")


def stage(found: dict, home: str) -> Path:
    """Download the release, check it, and copy its app next to the running one, hidden. Nothing running
    changes; install() puts it in place."""
    app = Path(found["app"])
    if not os.access(app.parent, os.W_OK):
        raise UpdateError(f"Zana's folder, {app.parent}, can't be written; move Zana to Applications first")
    team, bundle_id = team_id(app), info(app)["CFBundleIdentifier"]
    work = Path(home) / "update"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    dmg, mount = work / "Zana.dmg", work / "disk"
    run("curl", "-fsSL", "-m", "600", "-o", str(dmg), found["dmg"], timeout=660)
    want = run("curl", "-fsSL", "-m", "30", found["sha256"]).split()[0].lower()
    with open(dmg, "rb") as f:
        if hashlib.file_digest(f, "sha256").hexdigest() != want:
            raise UpdateError("the download doesn't match its checksum")
    run("hdiutil", "attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint", str(mount), str(dmg))
    staged = app.with_name(f".{app.stem}-update.app")
    try:
        apps = sorted(mount.glob("*.app"))
        if not apps:
            raise UpdateError("the disk image has no app in it")
        shutil.rmtree(staged, ignore_errors=True)
        run("ditto", str(apps[0]), str(staged))
    finally:
        subprocess.run(["hdiutil", "detach", str(mount), "-force"], capture_output=True, timeout=60, check=False)
        dmg.unlink(missing_ok=True)
    try:
        verify(staged, team, bundle_id, found["version"])
    except UpdateError:
        shutil.rmtree(staged, ignore_errors=True)
        raise
    return staged


def load_state(home: str) -> dict:
    try:
        with open(Path(home) / STATE) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def write_state(home: str, state: dict) -> None:
    tmp = Path(home) / f"{STATE}.tmp"
    tmp.write_text(json.dumps(state))
    tmp.replace(Path(home) / STATE)


def save_state(home: str, **changes) -> None:
    write_state(home, {**load_state(home), **changes})


def take_done(home: str) -> dict | None:
    """The update Zana just restarted for ({"from", "to"}), once: the restarted Voicebot says it."""
    state = load_state(home)
    done = state.pop("done", None)
    if done is not None:
        write_state(home, state)
    return done


def version(app: Path) -> str:
    """The app's version; ZANA_VERSION stands in for it in development, to try an update."""
    return os.getenv("ZANA_VERSION") or str(info(app)["CFBundleShortVersionString"])


def install(app: Path, staged: Path) -> None:
    """Put the checked copy in the app's place while Zana still runs (from its old files, set aside). Voicebot
    does this itself, inside Zana: macOS lets an app replace an app by the same developer, which a helper
    left running after Zana quits might not be allowed to do. A failure puts the old app back."""
    aside = app.with_name(f".{app.stem}-old.app")
    shutil.rmtree(aside, ignore_errors=True)
    try:
        os.rename(app, aside)
    except OSError as exc:
        shutil.rmtree(staged, ignore_errors=True)
        raise UpdateError(f"the old app couldn't be moved aside ({exc.strerror})") from None
    try:
        os.rename(staged, app)
    except OSError as exc:
        os.rename(aside, app)
        shutil.rmtree(staged, ignore_errors=True)
        raise UpdateError(f"the new app couldn't be moved in ({exc.strerror})") from None


def remove_old(app: Path) -> None:
    """The old app install() set aside; the restarted Voicebot removes it."""
    shutil.rmtree(app.with_name(f".{app.stem}-old.app"), ignore_errors=True)


def running(app: Path) -> bool:
    """Whether a Zana started from this path still runs (its command line keeps the path it started from). -a:
    this runs inside Zana's process tree, and pgrep leaves out the caller's ancestors without it."""
    found = subprocess.run(["pgrep", "-a", "-f", re.escape(f"{app}/Contents/MacOS/")], capture_output=True,
                           check=False)
    return found.returncode == 0


def quit_app(app: Path) -> None:
    """Quit Zana the normal way (it stops Voicebot); force it if it hangs (its next start clears leftovers)."""
    subprocess.run(["osascript", "-e", f'tell application id "{info(app)["CFBundleIdentifier"]}" to quit'],
                   capture_output=True, timeout=30, check=False)
    for wait, force in ((30, True), (10, False)):
        deadline = time.time() + wait
        while running(app) and time.time() < deadline:
            time.sleep(0.5)
        if not running(app):
            return
        if force:
            subprocess.run(["pkill", "-a", "-TERM", "-f", re.escape(f"{app}/Contents/MacOS/")],
                           capture_output=True, check=False)
    raise UpdateError("Zana didn't quit")


def launch_env() -> dict:
    """The environment Zana starts with when opened from the Dock. open hands its own environment to the app it
    opens, and this helper has Voicebot's (the old conversation's LiveKit key and secret, and in development
    the ZANA_* overrides), which must not carry over."""
    env = {k: os.environ[k] for k in ("HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "LANG") if k in os.environ}
    return {**env, "PATH": "/usr/bin:/bin:/usr/sbin:/sbin"}


def open_app(app: Path, tries: int = 10) -> None:
    """Open Zana and wait until it runs. Right after Zana quits, macOS can still count it as running for a
    moment, and opening it then does nothing or fails (-600), so it tries again."""
    for _ in range(tries):
        subprocess.run(["open", str(app)], env=launch_env(), capture_output=True, check=False)
        deadline = time.time() + 5
        while time.time() < deadline:
            if running(app):
                return
            time.sleep(0.5)
    raise UpdateError("Zana didn't open again")


def restart(app: Path, quit=quit_app, launch=open_app) -> None:
    """Quit Zana and open it again from the same path, now the new app."""
    print(time.strftime("%Y-%m-%d %H:%M:%S"), f"restarting {app}", flush=True)
    quit(app)
    print(time.strftime("%Y-%m-%d %H:%M:%S"), "Zana quit", flush=True)
    launch(app)
    print(time.strftime("%Y-%m-%d %H:%M:%S"), "Zana opened", flush=True)


def start_restart(app: Path, home: str) -> None:
    """Run restart() in a process of its own, in a new session: Zana stops Voicebot's process groups when it
    quits, and this has to outlive that. Its output goes to logs/update.log."""
    os.makedirs(Path(home) / "logs", exist_ok=True)
    with open(Path(home) / "logs" / "update.log", "a") as log:
        subprocess.Popen([sys.executable, os.path.abspath(__file__), "restart", str(app)], start_new_session=True,
                         cwd="/", stdin=subprocess.DEVNULL, stdout=log, stderr=log)


if __name__ == "__main__" and sys.argv[1:2] == ["restart"]:
    try:
        restart(Path(sys.argv[2]))
    except Exception as exc:  # noqa: BLE001 - logged; Zana is opened again either way
        print(time.strftime("%Y-%m-%d %H:%M:%S"), f"restart failed: {exc}", flush=True)
        subprocess.run(["open", sys.argv[2]], env=launch_env(), capture_output=True, check=False)
