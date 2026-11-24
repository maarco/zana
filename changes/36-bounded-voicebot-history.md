Voicebot now bounds operational log and prompt-backup storage: the Debug log keeps
up to 20 MB, each terminal-job log keeps up to 5 MB, and prompt history retains
20 versions. Transcript search reads bounded chunks off the agent event loop;
the transcript itself remains until the user deletes it.
