"""Zana updates: versions, the release's files, the running app, putting the new app in place, restarting.
Standard library only (run in CI); the macOS tools stay out of these tests."""
import json
import os
import plistlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
import updater  # noqa: E402

RELEASE = {"tag_name": "v0.1.6", "assets": [
    {"name": "Zana-v0.1.6-universal.dmg", "browser_download_url": "https://example.test/z.dmg"},
    {"name": "Zana-v0.1.6-universal.dmg.sha256", "browser_download_url": "https://example.test/z.dmg.sha256"},
    {"name": "notes.txt", "browser_download_url": "https://example.test/notes.txt"}]}


def make_app(path: Path, version: str, mark: str) -> Path:
    (path / "Contents" / "Resources" / "voicebot").mkdir(parents=True)
    with open(path / "Contents" / "Info.plist", "wb") as f:
        plistlib.dump({"CFBundleIdentifier": "app.zana", "CFBundleShortVersionString": version}, f)
    (path / "Contents" / "mark").write_text(mark)
    return path


class UpdaterTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        self.app = make_app(self.root / "Zana.app", "0.1.5", "old")
        self.staged = make_app(self.root / ".Zana-update.app", "0.1.6", "new")
        self.home = self.root / "home"
        self.home.mkdir()

    def tearDown(self):
        self.dir.cleanup()

    def test_only_a_higher_version_is_an_update(self):
        self.assertTrue(updater.newer("0.1.6", "0.1.5"))
        self.assertTrue(updater.newer("0.1.10", "0.1.9"))
        self.assertTrue(updater.newer("v0.2.0", "0.1.9"))
        self.assertFalse(updater.newer("0.1.5", "0.1.5"))
        self.assertFalse(updater.newer("0.1.4", "0.1.5"))

    def test_release_gives_its_disk_image_and_checksum(self):
        self.assertEqual(updater.parse_release(RELEASE), {
            "version": "0.1.6", "dmg": "https://example.test/z.dmg", "sha256": "https://example.test/z.dmg.sha256"})
        no_checksum = {"tag_name": "v0.1.6", "assets": RELEASE["assets"][:1]}
        with self.assertRaises(updater.UpdateError):
            updater.parse_release(no_checksum)

    def test_running_app_is_the_bundle_around_the_code(self):
        code = self.app / "Contents" / "Resources" / "voicebot"
        with mock.patch.dict(os.environ, {"ZANA_APP": ""}):
            self.assertEqual(updater.installed_app(str(code)), self.app.resolve())
            self.assertIsNone(updater.installed_app(str(self.home)))  # a source folder
        with mock.patch.dict(os.environ, {"ZANA_APP": str(self.app)}):
            self.assertEqual(updater.installed_app(str(self.home)), self.app)

    def test_check_compares_the_newest_release_with_the_running_version(self):
        code = str(self.app / "Contents" / "Resources" / "voicebot")
        with mock.patch.object(updater, "run", return_value=json.dumps(RELEASE)), \
                mock.patch.dict(os.environ, {"ZANA_APP": "", "ZANA_VERSION": ""}):
            found = updater.check(code)
            self.assertEqual((found["current"], found["version"], found["newer"]), ("0.1.5", "0.1.6", True))
            with mock.patch.dict(os.environ, {"ZANA_VERSION": "0.1.6"}):
                self.assertFalse(updater.check(code)["newer"])

    def test_install_puts_the_new_app_in_place_and_sets_the_old_one_aside(self):
        updater.install(self.app, self.staged)
        self.assertEqual((self.app / "Contents" / "mark").read_text(), "new")
        self.assertFalse(self.staged.exists())
        aside = self.root / ".Zana-old.app"
        self.assertEqual((aside / "Contents" / "mark").read_text(), "old")  # the running Zana's files
        updater.remove_old(self.app)  # after the restart
        self.assertFalse(aside.exists())

    def test_install_puts_the_old_app_back_when_the_new_one_cannot_move_in(self):
        with self.assertRaises(updater.UpdateError):
            updater.install(self.app, self.root / ".Zana-gone.app")
        self.assertEqual((self.app / "Contents" / "mark").read_text(), "old")
        self.assertFalse((self.root / ".Zana-old.app").exists())

    def test_running_sees_zana_from_inside_its_own_process_tree(self):
        # Voicebot and the restart helper run inside Zana's process tree, and macOS pgrep leaves out the caller's
        # ancestors unless asked (-a). Here a shell stands in for Zana (its command line starts with the app's
        # executable path) and its child asks whether Zana runs.
        env = dict(os.environ, APP=str(self.app), PY=sys.executable,
                   CODE="import sys, updater; from pathlib import Path; print(updater.running(Path(sys.argv[1])))")
        out = subprocess.run(["bash", "-c", 'exec -a "$APP/Contents/MacOS/Zana-app" /bin/sh -c \'"$PY" -c "$CODE" "$APP"; true\''],
                             env=env, cwd=os.path.join(os.path.dirname(__file__), ".."), capture_output=True, text=True,
                             timeout=30)
        self.assertEqual(out.stdout.strip(), "True", out.stderr)

    def test_reopened_zana_does_not_inherit_voicebot_environment(self):
        leaked = {"LIVEKIT_API_SECRET": "s", "LIVEKIT_API_KEY": "k", "ZANA_VERSION": "0.1.4", "HOME": "/Users/u"}
        with mock.patch.dict(os.environ, leaked):
            env = updater.launch_env()
        self.assertEqual(env["HOME"], "/Users/u")
        self.assertFalse({"LIVEKIT_API_SECRET", "LIVEKIT_API_KEY", "ZANA_VERSION"} & env.keys())

    def test_restart_quits_zana_then_opens_the_same_path(self):
        steps = []
        updater.restart(self.app, quit=lambda app: steps.append(("quit", app)),
                        launch=lambda app: steps.append(("open", app)))
        self.assertEqual(steps, [("quit", self.app), ("open", self.app)])

    def test_state_keeps_the_offered_version_beside_the_install_result(self):
        updater.save_state(str(self.home), offered="0.1.6")
        updater.save_state(str(self.home), done={"from": "0.1.5", "to": "0.1.6"})
        self.assertEqual(updater.take_done(str(self.home)), {"from": "0.1.5", "to": "0.1.6"})
        self.assertIsNone(updater.take_done(str(self.home)))  # said once
        self.assertEqual(updater.load_state(str(self.home)), {"offered": "0.1.6"})


if __name__ == "__main__":
    unittest.main()
