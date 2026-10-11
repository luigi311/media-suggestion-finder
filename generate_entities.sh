#!/usr/bin/env bash

DATABASE=$1

DATABASE_URL="sqlite://$DATABASE?mode=rwc" sea-orm-cli migrate up
DATABASE_URL="sqlite://$DATABASE?mode=rwc" sea-orm-cli generate entity --output-dir ./entity/src/ --entity-format dense --er-diagram --date-time-crate chrono --with-serde both
