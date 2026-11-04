# Lyric timing

Lyrics use the current audio generation's playback position. While a replacement track loads, its lyrics cannot accidentally follow the previous track's clock.

The audio worker publishes position at 25 ms intervals (previously 100 ms). Volume commands also publish the current position: continuous slider input previously prevented the receive timeout from firing and froze the displayed clock. Stream position continues to count consumed media samples rather than wall-clock time, so buffering silence and pausing do not advance the lyrics. Seeking updates the stream's media position and discards old PCM as before.

LRC offsets apply to the entire file, including metadata placed after timed lines. Positive offsets advance lyrics, negative offsets delay them. Empty timed cues clear the previous text during instrumental sections. Invalid/non-finite timestamps are ignored. Cue selection uses binary search and the actual timestamp, replacing an unconditional 120 ms advance.

Incoming and outgoing lyrics crossfade over 120 ms, replacing the incoming 360 ms transition that made correctly selected lines appear late. Reduced motion snaps directly to the current line.

The chat window no longer redraws its complete history on every main-capsule animation notification or every account polling tick. It redraws when its snapshot changes, when assets arrive, during interaction, and once per second to age activity labels. This removes unnecessary layout/paint work competing with lyric frames.

Regression coverage includes positive/negative offsets, UTF-8 BOMs, repeated timestamps, backwards position changes, empty timed lines and invalid timestamps. Existing stream tests cover buffering and discarding old PCM after seeking. Provider lyrics for a different recording/version can still have incorrect timings; the app cannot infer those corrections from LRC alone.
