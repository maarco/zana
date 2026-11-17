#!/usr/bin/env python3
"""Syntax-check the scripts of every src-ui/*.html page with `node --check`.

Each page's local <script src> files and inline scripts are checked together, in
page order, the way the browser runs them: a syntax error or two scripts
declaring the same global (`const LABELS`) silently kills the whole window (both
happened to Voicebot windows), so CI runs this.
Usage: python3 scripts/check_inline_js.py
"""
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent / "src-ui"

TAG = re.compile(r"<script([^>]*)>(.*?)</script>", re.S)

failed = 0
for html in sorted(ROOT.glob("*.html")):
    combined = []
    for attrs, body in TAG.findall(html.read_text()):
        src = re.search(r'src="([^"]+)"', attrs)
        kind = re.search(r'type="([^"]+)"', attrs)
        if kind and kind.group(1) not in ("text/javascript", "module"):
            continue  # data blocks such as text/x-dc
        if src:
            path = ROOT / src.group(1).split("?")[0]
            if src.group(1).startswith(("http:", "https:")) or not path.is_file():
                continue
            combined.append(f"// --- {src.group(1)}\n{path.read_text()}")
        elif body.strip():
            combined.append(f"// --- inline script\n{body}")
    if not combined:
        continue
    with tempfile.NamedTemporaryFile("w", suffix=".js", delete=False) as f:
        f.write("\n;\n".join(combined))
    result = subprocess.run(["node", "--check", f.name], capture_output=True, text=True)
    pathlib.Path(f.name).unlink()
    if result.returncode != 0:
        failed += 1
        print(f"{html.name}:\n{result.stderr.strip()}\n")
print(f"inline scripts: {'FAILED' if failed else 'ok'}")
sys.exit(1 if failed else 0)
