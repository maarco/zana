"""Import the shipped agent and prove its built-in tools are registered."""
import os
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VOICEBOT = ROOT / "voicebot"
REQUIRED_TOOLS = {
    "change_setting",
    "my_settings",
    "run_terminal",
    "see_screen",
    "set_mood",
    "type_up",
}


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="zana-agent-smoke-") as home:
        previous_home = os.environ.get("VOICEBOT_HOME")
        os.environ["VOICEBOT_HOME"] = home
        os.environ["VOICE_SERVER_URL"] = "http://127.0.0.1:9/v1"
        sys.path.insert(0, str(VOICEBOT))
        try:
            from agent import Voicebot

            bot = Voicebot(
                {"fillers": {"on": False}, "wake_word": "hey voicebot"},
                lambda _attributes: None,
            )
            tool_ids = {tool.id for tool in bot.tools}
        finally:
            if previous_home is None:
                os.environ.pop("VOICEBOT_HOME", None)
            else:
                os.environ["VOICEBOT_HOME"] = previous_home

    missing = REQUIRED_TOOLS - tool_ids
    if missing:
        raise SystemExit(f"missing built-in Voicebot tools: {sorted(missing)}")
    print(f"agent import smoke: {len(tool_ids)} tools registered")


if __name__ == "__main__":
    main()
