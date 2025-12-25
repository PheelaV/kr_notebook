# TTS Audio Generation for Korean Pronunciation

**Status**: Paused - Audio quality not production-ready

## Overview

The `machine-learning/` directory contains a Python TTS (Text-to-Speech) system for generating Korean pronunciation audio files. Two models were implemented:

1. **MMS-TTS** (Meta's Massively Multilingual Speech) - Fast, robotic voice
2. **CosyVoice2** (FunAudioLLM) - Zero-shot voice cloning, more natural but "scary"

## Current State

- 40 audio files generated for each model (all Hangul jamo characters)
- Audio files exist in `machine-learning/audio/hangul/{mms,cosyvoice}/`
- UI integration was removed due to poor audio quality

## Architecture

```
machine-learning/
├── pyproject.toml              # uv-managed Python project
├── src/kr_tts/
│   ├── __init__.py
│   ├── cli.py                  # CLI entry point (kr-tts command)
│   ├── generator.py            # Audio generation orchestration
│   └── models/
│       ├── base.py             # Abstract TTSModel interface
│       ├── mms.py              # Meta MMS-TTS implementation
│       └── cosyvoice.py        # CosyVoice2 implementation
├── CosyVoice/                  # Git submodule - CosyVoice repo
├── pretrained_models/          # Downloaded model weights (~3GB)
│   └── CosyVoice2-0.5B/
├── audio/hangul/               # Generated audio files
│   ├── mms/
│   └── cosyvoice/
└── reference_voice.wav         # Voice sample for CosyVoice cloning
```

## How It Works

### MMS-TTS Model
- Uses `facebook/mms-tts-kor` from HuggingFace
- Fixed Korean voice, no customization
- Fast inference on CPU/MPS/CUDA
- Single jamo characters (ㄱ, ㅏ) converted to pronounceable syllables via `JAMO_TO_PRONUNCIATION` mapping

### CosyVoice2 Model
- Uses `FunAudioLLM/CosyVoice2-0.5B` from HuggingFace
- Zero-shot voice cloning - requires a reference voice sample
- Clones the voice characteristics (timbre, pitch) from reference
- Can speak any language with the cloned voice
- Instruction-based control: "한국어로 천천히 또박또박 말해주세요"

### Jamo Pronunciation Mapping
Single Korean jamo cannot be synthesized directly. The system maps them:
```python
JAMO_TO_PRONUNCIATION = {
    "ㄱ": "기역",  # Consonant names
    "ㄴ": "니은",
    "ㅏ": "아",    # Vowels with silent ㅇ
    "ㅓ": "어",
    ...
}
```

## Usage

### Generate Audio Files
```bash
cd machine-learning

# MMS model (fast, robotic)
uv run kr-tts generate --model mms --device cpu

# CosyVoice model (slower, cloned voice)
uv run --group cosyvoice kr-tts generate --model cosyvoice --device cpu
```

### Test in Jupyter Notebook
```python
from IPython.display import Audio, display
from pathlib import Path
import tempfile

MODEL = "mms"  # or "cosyvoice"
TEXT = "안녕하세요"

if MODEL == "mms":
    from kr_tts.models.mms import MMSTTSModel
    model = MMSTTSModel(device="cpu")
else:
    from kr_tts.models.cosyvoice import CosyVoiceModel
    model = CosyVoiceModel(device="cpu")

with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
    output_path = Path(f.name)

model.synthesize(TEXT, output_path)
display(Audio(str(output_path), autoplay=True))
```

## Technical Notes

### torchaudio 2.9+ Compatibility
torchaudio 2.9+ ignores the `backend` parameter and tries to use torchcodec. CosyVoice is patched to use soundfile directly:
- `_load_wav_soundfile()` - Replacement for torchaudio.load
- `_patch_load_wav()` - Monkey-patches CosyVoice's file_utils

### Dependencies
The cosyvoice group has heavy dependencies:
- torch, torchaudio, transformers
- pyworld, librosa, soundfile
- omegaconf, hyperpyyaml
- Many others for the CosyVoice model

### Model Download
CosyVoice2-0.5B is ~3GB:
```bash
uv run huggingface-cli download FunAudioLLM/CosyVoice2-0.5B \
    --local-dir pretrained_models/CosyVoice2-0.5B
```

## Re-enabling in the App

To re-enable TTS in the Rust app:

1. **Restore audio buttons** in templates:
   - `templates/card.html`
   - `templates/practice_card.html`
   - `templates/library.html`

2. **Restore playAudio function** in `templates/base.html`

3. **Restore TTS settings** in `templates/settings.html`

4. **Symlink audio files**:
   ```bash
   ln -s /path/to/machine-learning/audio/hangul/mms static/audio/hangul/mms
   ```

## Future Improvements

1. **Better Voice Model**: Find or train a Korean-specific TTS model with natural pronunciation
2. **Edge TTS**: Consider Microsoft Edge TTS as alternative (cloud-based, better quality)
3. **Native Korean Voice**: Record or source a native Korean speaker for CosyVoice reference
4. **Selective TTS**: Only generate for commonly confused sounds, not all characters
