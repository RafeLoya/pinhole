#!/bin/bash

if [ -z "$1" ]; then
  echo "Usage: $0 logfile"
  exit 1
fi

LOGFILE="$1"

echo "Analyzing log: $LOGFILE"
echo "------------------------------------"

tcp_connections=$(grep -c "New TCP control connection" "$LOGFILE")
joins=$(grep -c "JOIN" "$LOGFILE")
connected=$(grep -c "CONNECTED" "$LOGFILE")
disconnected=$(grep -c "DISCONNECTED" "$LOGFILE")
no_peer_udp=$(grep -c "No peer found for UDP source" "$LOGFILE")
cleanups=$(grep -c "cleanup complete" "$LOGFILE")

echo "Total TCP control connections:     $tcp_connections"
echo "Total JOINs:                       $joins"
echo "Total CONNECTED events:            $connected"
echo "Total DISCONNECTED events:         $disconnected"
echo "UDP packets with no peer:          $no_peer_udp"
echo "Total client cleanup completions:  $cleanups"

echo
echo "Session join breakdown:"
grep "JOIN" "$LOGFILE" | awk '{print $7}' | sort | uniq -c | awk '{print "  Session " $2 ": " $1 " JOIN(s)"}'

echo
echo "Disconnected clients:"
grep "DISCONNECTED" "$LOGFILE" | awk -F'Sending to ' '{print $2}' | cut -d':' -f1-2 | sort | uniq
