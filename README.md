# avisafe-djilog-parser

Fly.io-app som parser DJI-flylogger (.txt og .zip), inkludert krypterte v13+
logger fra DJI Fly / Pilot 2 (Mavic 3, Matrice 30/300/350/400, Mini 4 Pro osv.).

Bygget på [`dji-log-parser`](https://github.com/lvauvillier/dji-log-parser) (Rust).

## Endepunkter

- `GET /health` — healthcheck for Fly.
- `POST /parse` — multipart med:
  - `file` — DJI `.txt` eller `.zip`
  - `format` — `json` (default) eller `csv`

  Krever `Authorization: Bearer ${AVISAFE_PARSER_TOKEN}`.

## Respons (json)

```json
{
  "ok": true,
  "version": 13,
  "details": { ... },
  "frames": [ { "custom": {...}, "osd": {...}, "gimbal": {...}, "camera": {...},
                "rc": {...}, "battery": {...}, "home": {...}, "recover": {...},
                "app": {...} }, ... ],
  "frame_count": 4823
}
```

## Krav for v13+ logger

For nyere DJI-firmware (logversjon ≥ 13) er records kryptert. Parseren må
hente per-fil keychains fra DJI sin offisielle API. Sett:

```
fly secrets set DJI_API_KEY=<din-api-nøkkel> --app avisafe-djilog-parser
fly secrets set AVISAFE_PARSER_TOKEN=<random-streng> --app avisafe-djilog-parser
```

API-nøkkel skaffes fra https://developer.dji.com/.

For eldre logger (v < 13) trengs ikke `DJI_API_KEY`.

## Deploy

```
cd dji-parser
fly deploy --app avisafe-djilog-parser
```

## Feilkoder

- `401` — ugyldig/manglende bearer token
- `422 reason: "v13 log requires DJI_API_KEY (not configured)"` — sett secret
- `422 reason: "fetch_keychains: ..."` — DJI API avviste nøkkel/log
- `422 reason: "from_bytes: ..."` — filen er ikke en DJI-logg
- `422 reason: "frames: ..."` — dekrypterte records, men frame-konvertering feilet
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
