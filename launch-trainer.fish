#!/usr/bin/env fish

# Proton Trainer Launcher - Launch Windows trainers in Steam Proton prefixes
# https://github.com/yourusername/proton-trainer-launcher

set -g TRAINER_PATH "/mnt/500Go-S850/Cheat/Aurora/Aurora.exe"
set -g STEAM_APPS "$HOME/.local/share/Steam/steamapps"
set -g COMPAT_TOOLS "$HOME/.local/share/Steam/compatibilitytools.d"

set -g COLOR_RESET (printf '\033[0m')
set -g COLOR_GREEN (printf '\033[32m')
set -g COLOR_YELLOW (printf '\033[33m')
set -g COLOR_RED (printf '\033[31m')
set -g COLOR_BLUE (printf '\033[34m')

function print_info
    printf '%s[INFO]%s %s\n' "$COLOR_BLUE" "$COLOR_RESET" "$argv"
end

function print_success
    printf '%s[OK]%s %s\n' "$COLOR_GREEN" "$COLOR_RESET" "$argv"
end

function print_warning
    printf '%s[WARN]%s %s\n' "$COLOR_YELLOW" "$COLOR_RESET" "$argv"
end

function print_error
    printf '%s[ERROR]%s %s\n' "$COLOR_RED" "$COLOR_RESET" "$argv"
end

function detect_running_games
    set -l running_games
    
    for pid in (pgrep -f "reaper SteamLaunch AppId=")
        set -l cmdline (cat /proc/$pid/cmdline 2>/dev/null | tr '\0' ' ')
        if string match -q "*AppId=*" $cmdline
            set -l app_id (string match -r 'AppId=(\d+)' $cmdline)[2]
            if test -n "$app_id"
                set -a running_games $app_id
            end
        end
    end
    
    printf '%s\n' $running_games | sort -u
end

function get_game_name
    set -l app_id $argv[1]
    set -l manifest "$STEAM_APPS/appmanifest_$app_id.acf"
    
    if test -f "$manifest"
        set -l name (grep '"name"' "$manifest" | head -n1 | sed 's/.*"\(.*\)".*/\1/')
        echo $name
    else
        echo "AppID $app_id"
    end
end

function detect_proton_version
    set -l app_id $argv[1]
    set -l compat_data "$STEAM_APPS/compatdata/$app_id"
    set -l version_file "$compat_data/version"
    
    if not test -f "$version_file"
        return 1
    end
    
    set -l version_info (cat "$version_file")
    
    if string match -qr 'GE-Proton([0-9]+-[0-9]+)' $version_info
        set -l ge_version (string match -r 'GE-Proton[0-9]+-[0-9]+' $version_info)
        set -l ge_path "$COMPAT_TOOLS/$ge_version"
        if test -d "$ge_path"
            echo "$ge_path"
            return 0
        end
    end
    
    if test -d "$COMPAT_TOOLS/proton-cachyos-10.0-20251126-slr-x86_64_v4"
        echo "$COMPAT_TOOLS/proton-cachyos-10.0-20251126-slr-x86_64_v4"
        return 0
    end
    
    return 1
end

function launch_trainer
    set -l app_id $argv[1]
    set -l delay $argv[2]
    set -l compat_data "$STEAM_APPS/compatdata/$app_id"
    
    if not test -d "$compat_data"
        print_error "Proton prefix for AppID $app_id does not exist"
        print_error "Path: $compat_data"
        return 1
    end
    
    set -l game_name (get_game_name $app_id)
    print_info "Prefix found for: $game_name"
    
    set -l proton_path (detect_proton_version $app_id)
    if test $status -ne 0
        print_error "Could not detect Proton version"
        return 1
    end
    
    print_success "Using Proton: "(basename $proton_path)
    
    if test -n "$delay" -a "$delay" -gt 0
        print_info "Waiting $delay seconds before launching trainer..."
        sleep $delay
    end
    
    print_info "Launching trainer in $game_name prefix..."
    
    env STEAM_COMPAT_DATA_PATH="$compat_data" \
        STEAM_COMPAT_CLIENT_INSTALL_PATH="$HOME/.local/share/Steam" \
        WINEPREFIX="$compat_data/pfx" \
        PROTON_NO_ESYNC=1 \
        PROTON_NO_FSYNC=1 \
        WINEESYNC=0 \
        WINEFSYNC=0 \
        SteamAppId=$app_id \
        SteamGameId=$app_id \
        "$proton_path/proton" run "$TRAINER_PATH" &
    
    set -l trainer_pid $last_pid
    print_success "Trainer launched with PID: $trainer_pid"
    print_info "Trainer is running in background"
end

function show_help
    echo "Usage: $argv[1] [OPTIONS] [APP_ID]"
    echo ""
    echo "Launch Windows game trainers in Steam Proton prefix"
    echo ""
    echo "Options:"
    echo "  APP_ID              Steam App ID of the game"
    echo "  --delay SECONDS     Wait before launching trainer"
    echo "  --list              List currently running Steam games"
    echo "  --help              Show this help"
    echo ""
    echo "Examples:"
    echo "  $argv[1]                     # Auto-detect running game"
    echo "  $argv[1] 241930              # Launch for specific game"
    echo "  $argv[1] --delay 5 241930    # Wait 5s then launch"
    echo "  $argv[1] --list              # Show running games"
end

function main
    if not test -f "$TRAINER_PATH"
        print_error "Trainer not found: $TRAINER_PATH"
        print_error "Edit TRAINER_PATH variable in the script"
        return 1
    end
    
    set -l app_id ""
    set -l delay 0
    set -l list_mode 0
    
    set -l i 1
    while test $i -le (count $argv)
        switch $argv[$i]
            case --help -h
                show_help $0
                return 0
            case --list -l
                set list_mode 1
            case --delay -d
                set i (math $i + 1)
                if test $i -le (count $argv)
                    set delay $argv[$i]
                else
                    print_error "Option --delay requires a value"
                    return 1
                end
            case '*'
                if test -z "$app_id"
                    set app_id $argv[$i]
                end
        end
        set i (math $i + 1)
    end
    
    if test $list_mode -eq 1
        set -l games (detect_running_games)
        if test (count $games) -eq 0
            print_warning "No Steam games currently running"
            return 0
        end
        
        print_success "Running games:"
        for game in $games
            set -l name (get_game_name $game)
            printf "  • AppID %s - %s\n" $game $name
        end
        return 0
    end
    
    if test -z "$app_id"
        print_info "Interactive mode: auto-detecting games..."
        set -l games (detect_running_games)
        
        if test (count $games) -eq 0
            print_warning "No Steam games detected"
            print_info "Launch your game first, then run: $0 [APP_ID]"
            return 1
        else if test (count $games) -eq 1
            set app_id $games[1]
            set -l name (get_game_name $app_id)
            print_success "Game detected: $name (AppID: $app_id)"
        else
            print_info "Multiple games detected:"
            for game in $games
                set -l name (get_game_name $game)
                printf "  • AppID %s - %s\n" $game $name
            end
            read -P "Enter App ID: " app_id
        end
    end
    
    if not string match -qr '^\d+$' $app_id
        print_error "Invalid App ID: $app_id"
        return 1
    end
    
    launch_trainer $app_id $delay
end

main $argv
