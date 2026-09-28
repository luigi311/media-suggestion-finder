#!/usr/bin/env bash

#rm database.db
#cargo build && ./target/debug/media-suggestion-finder ~/Videos

DATABASE=$1

DATABASE_URL="sqlite://$DATABASE?mode=rwc" sea-orm-cli migrate up
DATABASE_URL="sqlite://$DATABASE?mode=rwc" sea-orm-cli generate entity --output-dir ./entity/src/ --entity-format dense --er-diagram --date-time-crate chrono
