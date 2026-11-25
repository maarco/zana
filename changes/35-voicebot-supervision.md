# Voicebot supervision and recovery

After Voicebot is ready, Zana watches LiveKit, the speech server and the agent.
A dead child or a speech-health failure makes the orb leave Ready and restarts
the local Voicebot with bounded backoff. Three recovery failures in ten minutes
stop the loop with one actionable error instead of retrying forever.

The speech server reports unhealthy when one generation holds its global speech
lock for more than sixty seconds. Running terminal jobs are stopped during a
supervised shutdown, and their job number is disclosed when the job starts.
