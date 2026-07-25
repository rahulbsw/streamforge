# StreamForge product demo recording package

This is the canonical production plan for the next public StreamForge demo.
It is an internal recording document and is excluded from GitHub Pages.

## Publication gate

Do not record or publish the replacement until one clean local run proves all
of the following from the same UI-created pipeline:

1. The configuration validator exits successfully.
2. The UI and generated YAML name the same destination topic.
3. One synthetic input record is accepted.
4. The exact configured destination contains the expected transformed record.
5. Consumed, produced, delivered, and output counts all equal the expected
   record count.
6. Error and dead-letter counts remain zero.
7. The commit SHA, image tag, config, commands, and validation output are saved
   with the recording notes.

If any count or destination differs, stop. Do not edit around the failure or
join footage from separate pipelines.

## Storyboard: 75–90 seconds

| Time | Picture | Narration |
| --- | --- | --- |
| 0–6s | StreamForge mark and one source splitting into two shaped destinations | “StreamForge moves only the Kafka records and fields each destination needs.” |
| 6–16s | Tight terminal crop: validated example config | “Start with a versioned pipeline definition and validate it before deployment.” |
| 16–30s | UI form and generated YAML side by side | “Choose the source, define the destination filter and transform, then review the generated YAML.” |
| 30–42s | Deploy action followed by ready status | “The Kubernetes operator reconciles that definition into a running pipeline.” |
| 42–55s | Produce one synthetic order with a clearly visible identifier | “Publish one synthetic event so the complete path remains easy to verify.” |
| 55–72s | Destination consumer displays the same identifier with only the configured fields | “The configured destination receives the transformed representation—no separate verification topic.” |
| 72–82s | Health and four matching record counts; zero errors | “Health and delivery counters confirm the record was consumed and delivered exactly through this run.” |
| 82–90s | Documentation URL and quickstart command | “Run the same path locally from the StreamForge quickstart.” |

## Capture rules

- Capture the application or terminal window only at 1920×1080 and 30 fps.
- Use a large terminal font, high contrast, and a neutral background.
- Hide the desktop, menu bar, dock, notifications, bookmarks, account names,
  shell history, cloud identifiers, tokens, and real broker addresses.
- Use synthetic topics, records, and local-only credentials.
- Keep the pointer still unless it explains the current action.
- Record narration in a quiet track and create captions from the final edit.
- Do not include AWS, comparison charts, or performance numbers.

## Required outputs

- `streamforge-product-demo.mp4`: H.264 High Profile, `yuv420p`, 1080p30,
  AAC-LC audio at 48 kHz.
- `streamforge-product-demo-poster.png`: 1600×900 still showing the verified
  source-to-destination path.
- `streamforge-product-demo.vtt`: reviewed captions matching the final audio.
- `streamforge-product-demo-loop.webm`: silent 10–15 second README preview,
  cropped to the validation, produce, and transformed-output proof.
- `streamforge-product-demo-transcript.md`: accessible transcript plus the
  commit and validation metadata.

## Delivery encoding

Run these only after the validated master recording exists:

```bash
ffmpeg -i streamforge-product-demo-master.mov \
  -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,format=yuv420p" \
  -r 30 -c:v libx264 -profile:v high -crf 20 -movflags +faststart \
  -c:a aac -ar 48000 -b:a 160k streamforge-product-demo.mp4

ffmpeg -ss 00:00:58 -i streamforge-product-demo.mp4 \
  -frames:v 1 -vf "scale=1600:900:force_original_aspect_ratio=decrease,pad=1600:900:(ow-iw)/2:(oh-ih)/2" \
  streamforge-product-demo-poster.png
```

Review the encoded MP4 in Safari, Chrome, Firefox, and a mobile viewport before
replacing the archived UI preview.
