# GP AI Inbetween

AI-assisted inbetweening for Blender's Grease Pencil. Generate smooth animation frames between keyframes using ToonCrafter.

## Features

- **One-click generation**: Select two keyframes, click Generate, done
- **Confidence scoring**: Frames are scored and auto-accepted when high quality
- **Feedback learning**: Track what works to improve future generations
- **Cross-platform**: Windows, macOS, and Linux support
- **No Python dependencies**: Single binary, no pip install nightmares

## Installation

1. Download the latest release: `gp_ai_inbetween_vX.X.X.zip`
2. In Blender: Edit → Preferences → Add-ons → Install
3. Select the zip file
4. Enable "GP AI Inbetween"
5. Set your Replicate API key in the addon preferences

## Getting a Replicate API Key

1. Go to [replicate.com](https://replicate.com)
2. Sign up or log in
3. Go to Account Settings → API Tokens
4. Create a new token
5. Paste it in the addon preferences

## Usage

1. Select a Grease Pencil object
2. In the Timeline, select two keyframes (Shift+click)
3. In the sidebar (N panel), find "GP AI" tab
4. Click "Generate Inbetweens"
5. Wait ~30-60 seconds
6. Review generated frames with Accept/Reject buttons

## Configuration

In addon preferences you can set:

- **Replicate API Key**: Required for generation
- **Auto-Accept Threshold**: Confidence level for automatic acceptance (0.0-1.0)
- **Default Number of Frames**: How many inbetweens to generate by default

## CLI Usage

The Rust binary can also be used standalone:

```bash
# Generate frames
./gp_inbetween generate \
  --frame-a keyframe_001.png \
  --frame-b keyframe_010.png \
  --num-frames 4 \
  --output-dir ./output/

# View statistics
./gp_inbetween stats

# Generate default config
./gp_inbetween init-config
```

## Building from Source

Requirements:
- Rust 1.75+
- Cargo

```bash
# Clone the repo
git clone https://github.com/your-repo/gp-ai-inbetween
cd gp-ai-inbetween

# Build release binary
cargo build --release

# Binary is at target/release/gp_inbetween
```

## Project Structure

```
gp_inbetween/
├── cli/                    # CLI binary crate
│   └── src/main.rs
├── core/                   # Core library crate
│   └── src/
│       ├── lib.rs          # Main generator
│       ├── api.rs          # Replicate API client
│       ├── preprocessing.rs # Image preprocessing
│       ├── confidence.rs   # Frame scoring
│       ├── feedback.rs     # Usage logging
│       └── config.rs       # Configuration
├── blender_addon/          # Blender addon (Python)
│   ├── __init__.py
│   ├── operators.py
│   └── ui.py
└── .github/workflows/      # CI/CD
```

## How It Works

1. **Export**: Blender exports selected keyframes as PNG
2. **Preprocess**: Rust normalizes resolution and cleans up images
3. **Generate**: Calls ToonCrafter via Replicate API
4. **Score**: Each frame gets a confidence score
5. **Import**: Generated frames are imported back to Blender
6. **Track**: Accept/reject feedback improves future scoring

## Troubleshooting

### "API key not set"
Set your Replicate API key in addon preferences (Edit → Preferences → Add-ons → GP AI Inbetween)

### "Binary not found"
Ensure the addon was installed from the complete zip file that includes binaries in the `bin/` folder.

### Generation timeout
- Try generating fewer frames
- Check your internet connection
- Replicate may be under heavy load; try again later

### Poor quality results
- Ensure keyframes have clean strokes
- Use consistent art style between keyframes
- Avoid very complex motion (large rotations, multiple moving objects)

## License

MIT OR Apache-2.0

## Credits

- [ToonCrafter](https://github.com/ToonCrafter/ToonCrafter) for the AI model
- [Replicate](https://replicate.com) for model hosting
- [Blender](https://blender.org) for Grease Pencil
