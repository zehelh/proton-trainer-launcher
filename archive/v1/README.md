# Proton Trainer Launcher

Launch Windows game trainers (Aurora, WeMod, Plitch, etc.) for Steam games running under Proton on Linux.

## Features

- Auto-detects running Steam games
- Matches Proton version used by the game
- Supports multiple Proton variants (GE-Proton, Proton Experimental, CachyOS Proton)
- Interactive and manual modes
- Compatible with Aurora, WeMod, Plitch, and other Windows trainers

## Requirements

- Linux with Steam
- Proton or GE-Proton
- Windows trainer executable (Aurora, WeMod, Plitch, etc.)

## Compatibility

**Tested:**
- Aurora (Cheat Happens)

**Should work (untested):**
- WeMod
- Plitch
- FLiNG Trainers
- Any Windows trainer that attaches to game processes

## Installation

1. Clone or download this repository
2. Edit the `TRAINER_PATH` variable in the script to point to your trainer executable
3. Make the script executable:
   ```bash
   chmod +x launch-trainer.sh
   # or for fish
   chmod +x launch-trainer.fish
   ```

## Usage

### Auto-detect running game (recommended)
```bash
./launch-trainer.sh
```

### Specify game by App ID
```bash
./launch-trainer.sh 241930  # Shadow of Mordor
```

### With delay (if game takes time to load)
```bash
./launch-trainer.sh --delay 10 241930
```

### List running games
```bash
./launch-trainer.sh --list
```

## Configuration

Edit these variables at the top of the script:

```bash
TRAINER_PATH="/path/to/your/trainer.exe"  # Aurora.exe, WeMod.exe, Plitch.exe, etc.
STEAM_APPS="$HOME/.local/share/Steam/steamapps"
```

## Troubleshooting

### Game crashes when activating trainer options

1. **Use GE-Proton** instead of regular Proton (more permissive with memory modifications)
   - Download from [GE-Proton Releases](https://github.com/GloriousEggroll/proton-ge-custom/releases)
   - Extract to `~/.local/share/Steam/compatibilitytools.d/`
   - Set in Steam: Game Properties → Compatibility → Force GE-Proton

2. **Disable ESYNC/FSYNC** in Steam launch options:
   ```
   PROTON_NO_ESYNC=1 PROTON_NO_FSYNC=1 %command%
   ```

3. **Run game in windowed mode** (in-game graphics settings)

4. **Disable Steam overlay** in game properties

### Trainer doesn't attach to game

- Ensure the game is fully loaded before launching the trainer
- Try adding a delay: `./launch-trainer.sh --delay 10 <APP_ID>`

### Wine version mismatch error

Kill all Wine processes and retry:
```bash
wineserver -k
pkill wineserver
```

## Finding Game App IDs

- Check your Steam library URL: `steam://nav/games/details/<APP_ID>`
- Use SteamDB: https://steamdb.info/
- Run the script with `--list` while game is running

## Known Limitations

- Only works with games running under Proton
- Native Linux games are not supported
- Some anti-cheat protected games may not work

## License

MIT
