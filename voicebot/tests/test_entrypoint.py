"""The agent's entrypoint must not touch the room's local participant before the room connects
(session.start): LiveKit raises "cannot access local participant before connecting" and every
conversation job crashes. Standard library only (reads agent.py; run in CI)."""
import ast
import os
import unittest

AGENT = os.path.join(os.path.dirname(__file__), "..", "agent.py")


class EarlyAccess(ast.NodeVisitor):
    """Finds ctx.room.local_participant evaluated right away (lambdas and nested functions run later)."""

    def __init__(self):
        self.found = []

    def visit_Lambda(self, node):
        pass

    def visit_FunctionDef(self, node):
        pass

    visit_AsyncFunctionDef = visit_FunctionDef

    def visit_Attribute(self, node):
        if node.attr == "local_participant" and ast.unparse(node.value) == "ctx.room":
            self.found.append(node.lineno)
        self.generic_visit(node)


class EntrypointTest(unittest.TestCase):
    def test_nothing_touches_the_participant_before_the_room_connects(self):
        tree = ast.parse(open(AGENT).read())
        entry = next(n for n in ast.walk(tree) if isinstance(n, ast.AsyncFunctionDef) and n.name == "entrypoint")
        early = EarlyAccess()
        for stmt in entry.body:
            if "session.start(" in ast.unparse(stmt):
                break
            early.visit(stmt)
        else:
            self.fail("entrypoint never calls session.start")
        self.assertEqual(early.found, [], f"ctx.room.local_participant used before connecting at lines {early.found}")


if __name__ == "__main__":
    unittest.main()
