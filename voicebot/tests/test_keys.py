"""Key combos for the press_keys tool (agent.py imports livekit, so the helper is checked in isolation)."""
import ast
import os
import unittest

SOURCE = open(os.path.join(os.path.dirname(__file__), "..", "agent.py")).read()
NAMESPACE: dict = {}
for node in ast.parse(SOURCE).body:  # pull KEY_CODES, MODIFIERS and key_script out without importing livekit
    if (isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name)
            and node.targets[0].id in ("KEY_CODES", "MODIFIERS")) or \
            (isinstance(node, ast.FunctionDef) and node.name == "key_script"):
        exec(compile(ast.Module([node], []), "agent.py", "exec"), NAMESPACE)
key_script = NAMESPACE["key_script"]


class KeyScriptTest(unittest.TestCase):
    def test_named_keys_and_combos(self):
        self.assertEqual(key_script("enter"), 'tell application "System Events" to key code 36')
        self.assertEqual(key_script("cmd+v"), 'tell application "System Events" to keystroke "v" using {command down}')
        self.assertEqual(key_script("cmd+shift+t"),
                         'tell application "System Events" to keystroke "t" using {command down, shift down}')
        self.assertEqual(key_script("cmd+enter"), 'tell application "System Events" to key code 36 using {command down}')

    def test_rejects_unknown_keys_and_modifiers(self):
        for bad in ("hyper+a", "cmd+banana", ""):
            with self.assertRaises(ValueError):
                key_script(bad)


if __name__ == "__main__":
    unittest.main()
