# Voicebot process lifecycle

The speech server now asks the operating system for an available port and reports
its bound port back to Zana after startup, avoiding a stale free-port probe while
large local models load. Zana shuts down its LiveKit, speech-server, and agent
process groups concurrently under one four-second graceful-shutdown budget
before forcing any remaining groups to exit.

Long terminal jobs are explicitly disclosed in their start reply: the named job
will stop if the conversation ends (including orb close or Voicebot settings
changes), and cannot be resumed later. Explicit `stop_job` and the existing
15-minute job timeout also stop jobs.
