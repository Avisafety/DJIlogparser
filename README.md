# avisafe-djilog-parser

Fly.io-app som parser DJI-flylogger (.txt og .zip) og returnerer JSON i samme form som DroneLog API.

## Endepunkter

- `GET /health` — healthcheck for Fly.
- `POST /parse` — multipart med `file` (DJI .txt eller .zip) + valgfritt felt `fields` (kommaseparert liste). Krever `Authorization: Bearer ${AVISAFE_PARSER_TOKEN}`.

## Respons

```json
{
  "details": { "aircraftName": "...", "aircraftSN": "...", "startTime": "...", ... },
  "samples": [ { "t": 0, "OSD.latitude": 59.91, ..., "BATTERY.chargeLevel [%]": 87 }, ... ],
  "summary": { "maxAltitude": 120, "maxHSpeed": 12.4, "totalFlightTime": 537 }
}
```

## Deploy

```
fly deploy --app avisafe-djilog-parser
```

Sett secrets:
```
fly secrets set AVISAFE_PARSER_TOKEN=...
```

## Status

- v0.1: Stub-parser. Returnerer 422 for ukjente formater. Edge function fall-back til DroneLog `/logs/upload`.
- v0.2: Full DJI binærparser (TODO).
