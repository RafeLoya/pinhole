#!/bin/bash

if [ -z "$1" ]; then
  echo "Usage: $0 logfile"
  exit 1
fi

LOGFILE="$1"

echo "Analyzing log: $LOGFILE"
echo "------------------------------------"

# Basic counts
server_starts=$(grep -c "SFU server starting" "$LOGFILE")
tcp_connections=$(grep -c "New TCP control connection" "$LOGFILE")
joins=$(grep -c "JOIN" "$LOGFILE")
connected=$(grep -c "CONNECTED" "$LOGFILE")
disconnected=$(grep -c "DISCONNECTED" "$LOGFILE")
no_peer_udp=$(grep -c "No peer found for UDP source" "$LOGFILE")
cleanups=$(grep -c "cleanup complete" "$LOGFILE")

# Unique IPs (clients)
unique_clients=$(grep "New TCP control connection" "$LOGFILE" | awk '{print $NF}' | cut -d':' -f1 | sort | uniq | wc -l)

# Unique sessions (assumes session ID follows "JOIN ")
unique_sessions=$(grep "JOIN" "$LOGFILE" | sed -n 's/.*JOIN \([a-zA-Z0-9_-]*\).*/\1/p' | sort | uniq | wc -l)

# Output summary
printf "%-40s %d\n" "Total server starts:" "$server_starts"
printf "%-40s %d\n" "Total TCP control connections:" "$tcp_connections"
printf "%-40s %d\n" "Total JOINs:" "$joins"
printf "%-40s %d\n" "Total unique sessions:" "$unique_sessions"
printf "%-40s %d\n" "Total unique clients:" "$unique_clients"
printf "%-40s %d\n" "Total CONNECTED events:" "$connected"
printf "%-40s %d\n" "Total DISCONNECTED events:" "$disconnected"
printf "%-40s %d\n" "UDP packets with no peer:" "$no_peer_udp"
printf "%-40s %d\n" "Total client cleanup completions:" "$cleanups"

echo
echo "JOINs per session:"
grep "JOIN" "$LOGFILE" | sed -n 's/.*JOIN \([a-zA-Z0-9_-]*\).*/\1/p' | sort | uniq -c | awk '{printf "  Session %-15s : %s JOIN(s)\n", $2, $1}'

echo
echo "Disconnected clients:"
grep "DISCONNECTED" "$LOGFILE" | awk -F'Sending to ' '{print $2}' | cut -d':' -f1-2 | sort | uniq
