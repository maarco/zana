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
    """Why an update can't go ahead, in fixed words Voicebot can say. `detail` (tool output, which can echo
    names from the download or the release) is only logged, never given to the model."""

    def __init__(self, message: str, detail: str = "") -> None:
        super().__init__(message)
        self.detail = detail


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
        raise UpdateError(f"{Path(args[0]).name} took too long") from None
    if out.returncode:
        raise UpdateError(f"{Path(args[0]).name} failed", detail=(out.stderr or out.stdout).strip()[:500])
    return out.stdout + out.stderr


def parse_release(release: dict) -> dict:
    """The version of a GitHub release and the links to its disk image and that image's checksum."""
    try:
        assets = {str(a["name"]): str(a["browser_download_url"]) for a in release.get("assets") or []}
        version = str(release["tag_name"]).lstrip("vV")
    except (AttributeError, KeyError, TypeError):
        raise UpdateError("GitHub's answer about the newest release didn't make sense") from None
    # Only a plain version leaves this function: it goes into Voicebot's instructions, so no other text from
    # the release (names, notes) ever reaches the model.
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise UpdateError("GitHub's newest release doesn't have a plain version number")
    images = [name for name in assets if name.endswith(".dmg")]
    dmg = f"Zana-v{version}-universal.dmg" if f"Zana-v{version}-universal.dmg" in images else \
        (images[0] if len(images) == 1 else None)
    if not dmg or f"{dmg}.sha256" not in assets:
        raise UpdateError(f"release {version} has no single disk image with a checksum")
    return {"version": version, "dmg": assets[dmg], "sha256": assets[f"{dmg}.sha256"]}


def check(code_dir: str) -> dict:
    """The running app, its version and the newest release; "newer" says whether that release is an update."""
    app = installed_app(code_dir)
    if app is None:
        raise UpdateError("this copy of Zana runs from source, so it doesn't update itself")
    current = version(app)
    try:
        release = json.loads(run("curl", "-fsSL", "--proto", "=https", "-m", "20",
                                 "-H", "Accept: application/vnd.github+json",
                                 f"https://api.github.com/repos/{REPO}/releases/latest"))
    except (UpdateError, ValueError) as exc:
        raise UpdateError("GitHub didn't answer", detail=getattr(exc, "detail", "") or str(exc)) from None
    found = parse_release(release)
    return {"app": str(app), "current": current, **found, "newer": newer(found["version"], current)}


def team_id(app: Path) -> str:
    found = re.search(r"^TeamIdentifier=(\w+)$", run("codesign", "-dv", "--verbose=2", str(app)), re.MULTILINE)
    if not found or found.group(1) == "not":  # "TeamIdentifier=not set": an unsigned or ad-hoc build
        raise UpdateError("this copy of Zana isn't signed by its developer, so it can't vouch for an update")
    return found.group(1)


def verify(app: Path, team: str, bundle_id: str, want: str) -> None:
    """The same developer's signature (an Apple-issued certificate for the running app's team), accepted by
    Gatekeeper (notarized), and the app and version the release says. The messages are fixed: the tools' own
    output names files inside the download, which must not reach the model."""
    try:
        run("codesign", "--verify", "--deep", "--strict", "--test-requirement",
            f'=anchor apple generic and certificate leaf[subject.OU] = "{team}"', str(app))
    except UpdateError:
        raise UpdateError("the download isn't signed by Zana's developer") from None
    try:
        run("spctl", "--assess", "--type", "execute", str(app))
    except UpdateError:
        raise UpdateError("macOS doesn't accept the download (it isn't notarized)") from None
    plist = info(app)
    if plist.get("CFBundleIdentifier") != bundle_id or plist.get("CFBundleShortVersionString") != want:
        raise UpdateError(f"the download isn't Zana {want}")


def stage(found: dict, home: str) -> Path:
    """Download the release, check it, and copy its app next to the running one, hidden. Nothing running
    changes; install() puts it in place."""
    app = Path(found["app"])
    if not os.access(app.parent, os.W_OK):
        raise UpdateError(f"Zana's folder, {app.parent}, can't be written; move Zana to Applications first")
    team, bundle_id = team_id(app), info(app)["CFBundleIdentifier"]
    work = Path(home) / "update"
    dmg, mount = work / "Zana.dmg", work / "disk"
    detach(mount)  # left mounted by an update that crashed
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True, exist_ok=True)
    run("curl", "-fsSL", "--proto", "=https", "--max-filesize", "1000000000", "-m", "600", "-o", str(dmg),
        found["dmg"], timeout=660)
    want = (run("curl", "-fsSL", "--proto", "=https", "-m", "30", found["sha256"]).split() or [""])[0].lower()
    with open(dmg, "rb") as f:
        if hashlib.file_digest(f, "sha256").hexdigest() != want:
            raise UpdateError("the download doesn't match its checksum")
    staged = app.with_name(f".{app.stem}-update.app")
    shutil.rmtree(staged, ignore_errors=True)
    try:
        run("hdiutil", "attach", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint", str(mount), str(dmg))
        try:
            apps = sorted(mount.glob("*.app"))
            if not apps:
                raise UpdateError("the disk image has no app in it")
            run("ditto", str(apps[0]), str(staged))
        finally:
            detach(mount)
            dmg.unlink(missing_ok=True)
        verify(staged, team, bundle_id, found["version"])
    except BaseException:
        shutil.rmtree(staged, ignore_errors=True)  # a partial or rejected copy never stays next to Zana
        raise
    return staged


def detach(mount: Path) -> None:
    subprocess.run(["hdiutil", "detach", str(mount), "-force"], capture_output=True, timeout=60, check=False)


def may_install(state: dict, version: str, last_heard: float) -> bool:
    """Only a version the user was told about, and only once they have spoken since (their yes)."""
    return state.get("offered") == version and last_heard > float(state.get("offered_at") or 0)


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


def take_done(home: str, fresh: float = 900) -> dict | None:
    """The update Zana just restarted for ({"from", "to", "t"}), once: the restarted Voicebot says it. One older
    than a quarter hour (the restart never happened) is dropped unsaid."""
    state = load_state(home)
    done = state.pop("done", None)
    if done is not None:
        write_state(home, state)
    return done if done and time.time() - float(done.get("t") or 0) < fresh else None


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
    """The old app install() set aside (and a new one that failed to open); the restarted Voicebot removes them."""
    for leftover in (f".{app.stem}-old.app", f".{app.stem}-failed.app"):
        shutil.rmtree(app.with_name(leftover), ignore_errors=True)


def roll_back(app: Path) -> None:
    """Put the old app install() set aside back in place; the new one moves aside as failed."""
    aside, failed = app.with_name(f".{app.stem}-old.app"), app.with_name(f".{app.stem}-failed.app")
    if not aside.is_dir():
        raise UpdateError("the old app is gone")
    shutil.rmtree(failed, ignore_errors=True)
    os.rename(app, failed)
    os.rename(aside, app)


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


def restart(app: Path, quit=quit_app, launch=open_app, alive=running, settle: float = 20) -> None:
    """Quit Zana and open it again from the same path, now the new app. If the new app doesn't open, or
    doesn't stay open for a few seconds, the old app goes back and opens instead."""
    print(time.strftime("%Y-%m-%d %H:%M:%S"), f"restarting {app}", flush=True)
    quit(app)
    print(time.strftime("%Y-%m-%d %H:%M:%S"), "Zana quit", flush=True)
    try:
        launch(app)
        time.sleep(settle)
        if not alive(app):
            raise UpdateError("the new Zana didn't stay open")
    except Exception as exc:  # noqa: BLE001 - whatever went wrong, the old app goes back
        print(time.strftime("%Y-%m-%d %H:%M:%S"), f"{exc}; putting the old Zana back", flush=True)
        roll_back(app)
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
