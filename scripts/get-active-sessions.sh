#!/bin/bash

# Check if log file is provided
if [ $# -ne 1 ]; then
    echo "Usage: $0 <log_file>"
    exit 1
fi

LOG_FILE=$1

# Check if the file exists
if [ ! -f "$LOG_FILE" ]; then
    echo "Error: Log file $LOG_FILE does not exist!"
    exit 1
fi

# Extract only lines after the most recent "SFU server starting"
TMP_FILTERED_LOG=$(mktemp)
awk '/SFU server starting/ { last = NR } { if (NR >= last) print }' "$LOG_FILE" > "$TMP_FILTERED_LOG"

# Temporary files to track session data
CLIENT_SESSION_FILE=$(mktemp)
SESSION_COUNT_FILE=$(mktemp)
SESSION_CLIENTS_FILE=$(mktemp)

# Clean up temp files on exit
trap 'rm -f "$CLIENT_SESSION_FILE" "$SESSION_COUNT_FILE" "$SESSION_CLIENTS_FILE" "$TMP_FILTERED_LOG"' EXIT

# Process the filtered log file
while IFS= read -r line; do
    if [ -z "$line" ]; then
        continue
    fi

    timestamp=$(echo "$line" | awk '{print $1, $2}')

    if echo "$line" | grep -q "joined session"; then
        client=$(echo "$line" | grep -oE "[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+:[0-9]+" | head -1)
        session_id=$(echo "$line" | grep -o "joined session \S\+" | awk '{print $3}')
        echo "$client $session_id" >> "$CLIENT_SESSION_FILE"
        echo "$session_id $client" >> "$SESSION_CLIENTS_FILE"

        if grep -q "^$session_id " "$SESSION_COUNT_FILE"; then
            count=$(grep "^$session_id " "$SESSION_COUNT_FILE" | awk '{print $2}')
            new_count=$((count + 1))
            sed -i '' "s/^$session_id .*/$session_id $new_count/" "$SESSION_COUNT_FILE"
        else
            echo "$session_id 1" >> "$SESSION_COUNT_FILE"
        fi
    fi

    if echo "$line" | grep -q "Session .* marked connected"; then
        session_id=$(echo "$line" | grep -o "Session \S\+ marked" | awk '{print $2}')
        # (optional handling if needed)
    fi

    if echo "$line" | grep -q "disconnected"; then
        client=$(echo "$line" | grep -oE "[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+:[0-9]+" | head -1)
        if grep -q "^$client " "$CLIENT_SESSION_FILE"; then
            session_id=$(grep "^$client " "$CLIENT_SESSION_FILE" | awk '{print $2}')
            grep -v "^$client " "$CLIENT_SESSION_FILE" > "${CLIENT_SESSION_FILE}.tmp"
            mv "${CLIENT_SESSION_FILE}.tmp" "$CLIENT_SESSION_FILE"

            if grep -q "^$session_id " "$SESSION_COUNT_FILE"; then
                count=$(grep "^$session_id " "$SESSION_COUNT_FILE" | awk '{print $2}')
                new_count=$((count - 1))
                if [ $new_count -le 0 ]; then
                    grep -v "^$session_id " "$SESSION_COUNT_FILE" > "${SESSION_COUNT_FILE}.tmp"
                    mv "${SESSION_COUNT_FILE}.tmp" "$SESSION_COUNT_FILE"
                else
                    sed -i '' "s/^$session_id .*/$session_id $new_count/" "$SESSION_COUNT_FILE"
                fi
            fi
        fi
    fi

done < "$TMP_FILTERED_LOG"

# Print final summary
echo -e "--- Active Sessions Summary ---"
if [ ! -s "$SESSION_COUNT_FILE" ]; then
    echo "No active sessions"
else
    while read -r session_id count; do
        echo "Session: $session_id ($count connections)"
        echo "  Connected clients:"
        grep "^$session_id " "$SESSION_CLIENTS_FILE" | awk '{print $2}' | while read -r client; do
            if grep -q "^$client " "$CLIENT_SESSION_FILE"; then
                echo "    - $client"
            fi
        done
    done < "$SESSION_COUNT_FILE"
fi
