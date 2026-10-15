#!/usr/bin/env bash

# Proton Trainer Launcher - Launch Windows trainers in Steam Proton prefixes
# https://github.com/yourusername/proton-trainer-launcher

set -euo pipefail

TRAINER_PATH="/mnt/500Go-S850/Cheat/Aurora/Aurora.exe"
STEAM_APPS="$HOME/.local/share/Steam/steamapps"

COLOR_RESET='\033[0m'
COLOR_GREEN='\033[32m'
COLOR_YELLOW='\033[33m'
COLOR_RED='\033[31m'
COLOR_BLUE='\033[34m'

print_info() { echo -e "${COLOR_BLUE}[INFO]${COLOR_RESET} $*"; }
print_success() { echo -e "${COLOR_GREEN}[OK]${COLOR_RESET} $*"; }
print_warning() { echo -e "${COLOR_YELLOW}[WARN]${COLOR_RESET} $*"; }
print_error() { echo -e "${COLOR_RED}[ERROR]${COLOR_RESET} $*"; }

detect_running_games() {
    pgrep -f "reaper SteamLaunch AppId=" | while read -r pid; do
        cmdline=$(tr '\0' ' ' < /proc/"$pid"/cmdline 2>/dev/null || true)
        if [[ $cmdline =~ AppId=([0-9]+) ]]; then
            echo "${BASH_REMATCH[1]}"
        fi
    done | sort -u
}

get_game_name() {
    local app_id=$1
    local manifest="$STEAM_APPS/appmanifest_${app_id}.acf"
    
    if [[ -f $manifest ]]; then
        grep '"name"' "$manifest" | head -n1 | sed 's/.*"\(.*\)".*/\1/'
    else
        echo "AppID $app_id"
    fi
}

detect_proton_version() {
    local app_id=$1
    local compat_data="$STEAM_APPS/compatdata/$app_id"
    compat_tool="$(sed '3!d' "$compat_data"/config_info)"
    echo "${compat_tool:0:-11}"
}

launch_trainer() {
    local app_id=$1
    local delay=${2:-0}
    local compat_data="$STEAM_APPS/compatdata/$app_id"
    
    if [[ ! -d $compat_data ]]; then
        print_error "Proton prefix for AppID $app_id does not exist"
        print_error "Path: $compat_data"
        return 1
    fi
    
    local game_name
    game_name=$(get_game_name "$app_id")
    print_info "Prefix found for: $game_name"
    
    local proton_path
    if proton_path=$(detect_proton_version "$app_id"); then
        print_success "Using Proton: $(basename "$proton_path")"
    else
        print_error "Could not detect Proton version"
        return 1
    fi
    
    if [[ $delay -gt 0 ]]; then
        print_info "Waiting $delay seconds before launching trainer..."
        sleep "$delay"
    fi
    
    print_info "Launching trainer in $game_name prefix..."
    
    STEAM_COMPAT_DATA_PATH="$compat_data" \
    STEAM_COMPAT_CLIENT_INSTALL_PATH="$HOME/.local/share/Steam" \
    WINEPREFIX="$compat_data/pfx" \
    PROTON_NO_ESYNC=1 \
    PROTON_NO_FSYNC=1 \
    WINEESYNC=0 \
    WINEFSYNC=0 \
    SteamAppId="$app_id" \
    SteamGameId="$app_id" \
    "$proton_path/proton" run "$TRAINER_PATH" &
    
    local trainer_pid=$!
    print_success "Trainer launched with PID: $trainer_pid"
    print_info "Trainer is running in background"
}

show_help() {
    cat << EOF
Usage: $0 [OPTIONS] [APP_ID]

Launch Windows game trainers in Steam Proton prefix

Options:
  APP_ID              Steam App ID of the game (e.g., 241930 for Shadow of Mordor)
  --delay SECONDS     Wait before launching trainer
  --list              List currently running Steam games
  --help              Show this help

Examples:
  $0                     # Auto-detect running game
  $0 241930              # Launch for specific game
  $0 --delay 5 241930    # Wait 5s then launch
  $0 --list              # Show running games
EOF
}

main() {
    if [[ ! -f $TRAINER_PATH ]]; then
        print_error "Trainer not found: $TRAINER_PATH"
        print_error "Edit TRAINER_PATH variable in the script"
        return 1
    fi
    
    local app_id=""
    local delay=0
    local list_mode=0
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --help|-h)
                show_help
                return 0
                ;;
            --list|-l)
                list_mode=1
                shift
                ;;
            --delay|-d)
                delay=$2
                shift 2
                ;;
            *)
                if [[ -z $app_id ]]; then
                    app_id=$1
                fi
                shift
                ;;
        esac
    done
    
    if [[ $list_mode -eq 1 ]]; then
        local games
        mapfile -t games < <(detect_running_games)
        
        if [[ ${#games[@]} -eq 0 ]]; then
            print_warning "No Steam games currently running"
            return 0
        fi
        
        print_success "Running games:"
        for game in "${games[@]}"; do
            local name
            name=$(get_game_name "$game")
            echo "  • AppID $game - $name"
        done
        return 0
    fi
    
    if [[ -z $app_id ]]; then
        print_info "Interactive mode: auto-detecting games..."
        local games
        mapfile -t games < <(detect_running_games)
        
        if [[ ${#games[@]} -eq 0 ]]; then
            print_warning "No Steam games detected"
            print_info "Launch your game first, then run: $0 [APP_ID]"
            return 1
        elif [[ ${#games[@]} -eq 1 ]]; then
            app_id=${games[0]}
            local name
            name=$(get_game_name "$app_id")
            print_success "Game detected: $name (AppID: $app_id)"
        else
            print_info "Multiple games detected:"
            for game in "${games[@]}"; do
                local name
                name=$(get_game_name "$game")
                echo "  • AppID $game - $name"
            done
            read -rp "Enter App ID: " app_id
        fi
    fi
    
    if [[ ! $app_id =~ ^[0-9]+$ ]]; then
        print_error "Invalid App ID: $app_id"
        return 1
    fi
    
    launch_trainer "$app_id" "$delay"
}

main "$@"
