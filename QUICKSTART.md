# Quick Start

## 1. Download

Clone or download this repository to your Linux machine.

## 2. Configure

Edit the trainer path in your chosen script (bash or fish):

```bash
# Bash
nano launch-trainer.sh
# Change: TRAINER_PATH="/path/to/your/trainer.exe"

# Fish
nano launch-trainer.fish
# Change: set -g TRAINER_PATH "/path/to/your/trainer.exe"
```

## 3. Make Executable

```bash
chmod +x launch-trainer.sh
# or
chmod +x launch-trainer.fish
```

## 4. Run

Launch your Steam game first, then:

```bash
./launch-trainer.sh
# or
./launch-trainer.fish
```

The script will auto-detect your running game and launch the trainer in the correct Proton environment.

## Troubleshooting

**Game crashes when using trainer?**
- Use GE-Proton instead of default Proton
- Add `PROTON_NO_ESYNC=1 PROTON_NO_FSYNC=1 %command%` to Steam launch options

**Trainer won't start?**
- Kill Wine processes: `wineserver -k && pkill wineserver`
- Verify trainer path is correct
- Check game is running before launching trainer

## Get App ID

If auto-detection fails, find your game's App ID:
- Steam library URL: `steam://nav/games/details/<APP_ID>`
- SteamDB: https://steamdb.info/

Then run: `./launch-trainer.sh <APP_ID>`
