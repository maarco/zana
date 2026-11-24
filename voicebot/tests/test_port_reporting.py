"""Tests for the speech server's dynamically assigned port handoff."""
import os
import socket
import sys
import tempfile
import unittest
from types import SimpleNamespace

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from port_reporting import report_bound_port  # noqa: E402


class PortReportingTest(unittest.TestCase):
    def test_writes_the_bound_port_atomically(self):
        with tempfile.TemporaryDirectory() as home, socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            expected_port = listener.getsockname()[1]
            output = os.path.join(home, "voice", "server.port")
            server = SimpleNamespace(
                started=True,
                servers=[SimpleNamespace(sockets=[listener])],
            )

            self.assertEqual(report_bound_port(server, output), expected_port)
            with open(output, encoding="ascii") as f:
                self.assertEqual(f.read().strip(), str(expected_port))
            self.assertEqual(os.listdir(os.path.dirname(output)), ["server.port"])

    def test_refuses_to_report_before_listen(self):
        with tempfile.TemporaryDirectory() as home:
            output = os.path.join(home, "server.port")
            server = SimpleNamespace(started=False, servers=[])
            with self.assertRaisesRegex(RuntimeError, "before the server has started"):
                report_bound_port(server, output)
            self.assertFalse(os.path.exists(output))


if __name__ == "__main__":
    unittest.main()
