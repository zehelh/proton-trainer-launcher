# Proton Trainer Launcher

## Example Configuration

Copy one of the scripts and edit the `TRAINER_PATH` variable:

### Bash
```bash
TRAINER_PATH="/path/to/your/trainer.exe"
```

### Fish
```fish
set -g TRAINER_PATH "/path/to/your/trainer.exe"
```

## Common Trainer Paths

- **Aurora**: `/path/to/Aurora.exe`
- **WeMod**: `$HOME/.local/share/WeMod/WeMod.exe` (if installed via Proton)
- **Plitch**: `/path/to/PLITCH.exe`
- **FLiNG**: `/path/to/trainer_name.exe`

## Notes

- Ensure your trainer executable path is absolute
- For trainers in Windows-style paths, use Linux path format
- If trainer is in a Proton prefix, use the Linux mount point
