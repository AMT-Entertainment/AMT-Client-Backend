#!/bin/bash
# Ping the AMT Social Server every 10 minutes to keep it alive on Render's free plan
# Add to crontab: crontab -e
# Then add: */10 * * * * /path/to/ping-social.sh

URL="https://amt-client-backend.onrender.com/health"

curl -s -o /dev/null -w "%{http_code}" "$URL" | grep -q 200 && echo "OK" || echo "FAIL"
