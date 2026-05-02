/**
 * DJI Flight Record .txt-parser.
 *
 * Format (forenklet, basert på offentlig reverse-engineering — datcon, CsvView):
 *
 *   [Records-blokk]                bytes 0 .. detailsOffset-1
 *   [Details-blokk]                bytes detailsOffset .. fileLen - 100
 *   [Footer 100 bytes]             siste 100 bytes — inneholder bl.a. detailsOffset (LE u64) på offset 0
 *
 * Hver record:
 *   1 byte  type
 *   1 byte  length
 *   N bytes payload
 *   1 byte  0xFF terminator
 *
 * Nyere DJI-firmware (post-2020, Mavic 3, Matrice 30/300/400) bruker scrambled
 * payloads som krever en AES/keychain-flow vi ikke har. For disse returnerer
 * vi { unsupported: true } slik at edge function faller tilbake til DroneLog.
 */

import type { ParseResult, Sample, Details } from "../types.js";

const REC_OSD = 1;
const REC_HOME = 2;
const REC_GIMBAL = 3;
const REC_RC = 4;
const REC_CUSTOM = 5;
const REC_DEFORM = 6;
const REC_CENTER_BATTERY = 7;
const REC_SMART_BATTERY = 8;
const REC_APP_TIP = 9;
const REC_APP_WARN = 10;
const REC_RC_GPS = 11;
const REC_RC_DEBUG = 12;
const REC_RECOVER = 13;
const REC_APP_GPS = 14;
const REC_FIRMWARE = 15;
const REC_OFDM_DEBUG = 16;
const REC_VISION_GROUP = 17;
const REC_VISION_WARN = 18;
const REC_MC_PARAM = 19;
const REC_APP_OPERATION = 20;
// 50+ er scrambled i nyere firmware

interface ParseOptions {
  fields: string[];
  // sampling: behold maks N samples (default 5000) — log kan ha 100k+ records
  maxSamples?: number;
}

export interface InternalParseResult {
  unsupported?: boolean;
  reason?: string;
  result?: ParseResult;
}

export function parseDjiTxt(
  buffer: Buffer,
  opts: ParseOptions,
): InternalParseResult {
  if (buffer.length < 200) {
    return { unsupported: true, reason: "file too small" };
  }

  // Footer-offset: siste 100 bytes inneholder details-offset i LE u64 på pos 0.
  const footerStart = buffer.length - 100;
  const detailsOffset = Number(buffer.readBigUInt64LE(footerStart));

  if (
    detailsOffset <= 0 ||
    detailsOffset >= footerStart ||
    detailsOffset > buffer.length - 100
  ) {
    return {
      unsupported: true,
      reason: `invalid detailsOffset=${detailsOffset} (fileLen=${buffer.length})`,
    };
  }

  const recordsBlock = buffer.subarray(0, detailsOffset);
  const detailsBlock = buffer.subarray(detailsOffset, footerStart);

  // Steg 1: detect scrambled
  // I scrambled logger ser vi sjelden 0xFF-terminator etter erklært lengde.
  // Vi gjør en kjapp sniff: les 200 records, sjekk hvor mange som har gyldig terminator.
  const scrambleScore = sniffScrambled(recordsBlock);
  if (scrambleScore > 0.5) {
    return {
      unsupported: true,
      reason: `scrambled records detected (score=${scrambleScore.toFixed(2)})`,
    };
  }

  const samples: Sample[] = [];
  const maxSamples = opts.maxSamples ?? 5000;

  let pos = 0;
  let osdCount = 0;
  let stride = 1; // settes etter første pass
  let lastSample: Partial<Sample> = {};
  let homeLat: number | undefined;
  let homeLon: number | undefined;
  let homeAlt: number | undefined;
  let gimbalPitch: number | undefined;
  let gimbalRoll: number | undefined;
  let gimbalYaw: number | undefined;
  let battery: Partial<Sample> = {};

  // Først teller vi OSD-records for å sette stride
  let osdTotal = 0;
  {
    let p = 0;
    while (p + 2 < recordsBlock.length) {
      const type = recordsBlock[p];
      const len = recordsBlock[p + 1];
      const end = p + 2 + len;
      if (end >= recordsBlock.length) break;
      if (recordsBlock[end] !== 0xff) {
        // resync: hopp en byte
        p += 1;
        continue;
      }
      if (type === REC_OSD) osdTotal++;
      p = end + 1;
    }
  }
  if (osdTotal === 0) {
    return {
      unsupported: true,
      reason: "no OSD records found (possibly scrambled or unknown version)",
    };
  }
  stride = Math.max(1, Math.floor(osdTotal / maxSamples));

  pos = 0;
  while (pos + 2 < recordsBlock.length) {
    const type = recordsBlock[pos];
    const len = recordsBlock[pos + 1];
    const end = pos + 2 + len;
    if (end >= recordsBlock.length) break;
    if (recordsBlock[end] !== 0xff) {
      pos += 1;
      continue;
    }
    const payload = recordsBlock.subarray(pos + 2, end);

    try {
      switch (type) {
        case REC_HOME:
          if (payload.length >= 24) {
            homeLon = (payload.readDoubleLE(0) * 180) / Math.PI;
            homeLat = (payload.readDoubleLE(8) * 180) / Math.PI;
            homeAlt = payload.readFloatLE(16);
          }
          break;

        case REC_GIMBAL:
          if (payload.length >= 6) {
            gimbalPitch = payload.readInt16LE(0) / 10;
            gimbalRoll = payload.readInt16LE(2) / 10;
            gimbalYaw = payload.readInt16LE(4) / 10;
          }
          break;

        case REC_SMART_BATTERY:
        case REC_CENTER_BATTERY:
          if (payload.length >= 30) {
            battery["BATTERY.totalVoltage [V]"] =
              payload.readUInt16LE(2) / 1000;
            battery["BATTERY.current [A]"] = payload.readInt16LE(4) / 1000;
            battery["BATTERY.chargeLevel [%]"] = payload.readUInt8(8);
            battery["BATTERY.temperature [°C]"] =
              payload.readInt16LE(10) / 10;
            // celler: opp til 6 stk, 2 bytes hver fra offset 12
            const cells: number[] = [];
            for (let i = 0; i < 6; i++) {
              const off = 12 + i * 2;
              if (off + 2 <= payload.length) {
                const v = payload.readUInt16LE(off) / 1000;
                if (v > 0.5 && v < 5) {
                  cells.push(v);
                  battery[`BATTERY.cellVoltage${i + 1} [V]`] = v;
                }
              }
            }
            if (cells.length >= 2) {
              const dev = Math.max(...cells) - Math.min(...cells);
              battery["BATTERY.cellVoltageDeviation [V]"] = dev;
            }
          }
          break;

        case REC_OSD:
          if (payload.length >= 50) {
            const lon = (payload.readDoubleLE(0) * 180) / Math.PI;
            const lat = (payload.readDoubleLE(8) * 180) / Math.PI;
            const altMsl = payload.readInt16LE(16) / 10;
            const heightAgl = payload.readInt16LE(18) / 10;
            const xSpeed = payload.readInt16LE(20) / 10;
            const ySpeed = payload.readInt16LE(22) / 10;
            const zSpeed = payload.readInt16LE(24) / 10;
            const pitch = payload.readInt16LE(26) / 10;
            const roll = payload.readInt16LE(28) / 10;
            const yaw = payload.readInt16LE(30) / 10;
            const flycState = payload.readUInt8(43);
            const gpsNum = payload.readUInt8(44);
            const flyTimeMs = osdCount * 100; // typisk 10 Hz

            const sample: Sample = {
              t: flyTimeMs,
              "OSD.latitude": lat,
              "OSD.longitude": lon,
              "OSD.altitude [m]": altMsl,
              "OSD.height [m]": heightAgl,
              "OSD.flyTime [ms]": flyTimeMs,
              "OSD.hSpeed [m/s]": Math.sqrt(
                xSpeed * xSpeed + ySpeed * ySpeed,
              ),
              "OSD.vSpeed [m/s]": -zSpeed,
              "OSD.pitch [°]": pitch,
              "OSD.roll [°]": roll,
              "OSD.directionYaw [°]": yaw,
              "OSD.gpsNum": gpsNum,
              "OSD.flycState": flycState,
            };
            if (homeLat !== undefined) {
              sample["HOME.latitude"] = homeLat;
              sample["HOME.longitude"] = homeLon;
              sample["HOME.altitude [m]"] = homeAlt;
            }
            if (gimbalPitch !== undefined) {
              sample["GIMBAL.pitch [°]"] = gimbalPitch;
              sample["GIMBAL.roll [°]"] = gimbalRoll;
              sample["GIMBAL.yaw [°]"] = gimbalYaw;
            }
            Object.assign(sample, battery);

            if (osdCount % stride === 0 && samples.length < maxSamples) {
              samples.push(sample);
            }
            lastSample = sample;
            osdCount++;
          }
          break;
      }
    } catch {
      // ignore corrupted record, continue
    }
    pos = end + 1;
  }

  if (samples.length === 0) {
    return {
      unsupported: true,
      reason: "parsed 0 samples from records",
    };
  }

  // Details-blokk (ASCII / ProtoBuf-ish). Vi gjør en best-effort tekstuttrekk.
  const details = parseDetails(detailsBlock);

  // Beregn summary + CALC
  let maxAltitude = 0;
  let maxHSpeed = 0;
  let maxVSpeed = 0;
  let maxDistance = 0;
  let totalDistance = 0;
  let prevLat: number | undefined;
  let prevLon: number | undefined;
  let maxCellDev = 0;

  for (const s of samples) {
    const alt = (s["OSD.altitude [m]"] as number) ?? 0;
    if (alt > maxAltitude) maxAltitude = alt;
    const h = (s["OSD.hSpeed [m/s]"] as number) ?? 0;
    if (h > maxHSpeed) maxHSpeed = h;
    const v = Math.abs((s["OSD.vSpeed [m/s]"] as number) ?? 0);
    if (v > maxVSpeed) maxVSpeed = v;
    const lat = s["OSD.latitude"] as number;
    const lon = s["OSD.longitude"] as number;
    if (homeLat !== undefined && lat) {
      const d2 = haversine(lat, lon, homeLat, homeLon!);
      if (d2 > maxDistance) maxDistance = d2;
      s["CALC.distance2D [m]"] = d2;
      s["CALC.distance3D [m]"] = Math.sqrt(d2 * d2 + alt * alt);
    }
    if (prevLat !== undefined && lat) {
      totalDistance += haversine(prevLat, prevLon!, lat, lon);
    }
    prevLat = lat;
    prevLon = lon;
    const dev = (s["BATTERY.cellVoltageDeviation [V]"] as number) ?? 0;
    if (dev > maxCellDev) maxCellDev = dev;
  }

  // Spre maxCellDev ut
  for (const s of samples) {
    s["BATTERY.maxCellVoltageDeviation [V]"] = maxCellDev;
    s["CALC.totalFlightTime [s]"] = (lastSample["OSD.flyTime [ms]"] as number) / 1000;
  }

  details["DETAILS.maxAltitude [m]"] = maxAltitude;
  details["DETAILS.maxHSpeed [m/s]"] = maxHSpeed;
  details["DETAILS.maxVSpeed [m/s]"] = maxVSpeed;
  details["DETAILS.maxDistance [m]"] = maxDistance;
  details["DETAILS.totalDistance [m]"] = totalDistance;
  details["DETAILS.totalTime [s]"] =
    ((lastSample["OSD.flyTime [ms]"] as number) ?? 0) / 1000;

  return {
    result: {
      details,
      samples: filterFields(samples, opts.fields),
      summary: {
        maxAltitude,
        maxHSpeed,
        maxVSpeed,
        maxDistance,
        totalDistance,
        totalFlightTime: details["DETAILS.totalTime [s]"] as number,
        sampleCount: samples.length,
      },
    },
  };
}

function sniffScrambled(block: Buffer): number {
  let tried = 0;
  let bad = 0;
  let p = 0;
  while (p + 2 < block.length && tried < 200) {
    const len = block[p + 1];
    const end = p + 2 + len;
    if (end >= block.length) break;
    if (block[end] !== 0xff) bad++;
    tried++;
    p = end + 1;
    if (block[end] !== 0xff) p = p - len; // resync mer aggressivt
    if (p < 0) p = 0;
  }
  if (tried === 0) return 1;
  return bad / tried;
}

function parseDetails(block: Buffer): Details {
  const details: Details = {};
  // Sniff ASCII strings ≥4 chars
  const text = block.toString("latin1");
  // DJI lagrer aircraftName, aircraftSN, fcSN, batterySN, cameraSN som NUL-terminerte strenger
  const stringMatches = text.match(/[\x20-\x7E]{4,32}/g) ?? [];
  // Ren heuristikk: SN-er er typisk 14-16 tegn, alfanumerisk
  const sns = stringMatches.filter((s) => /^[A-Z0-9]{10,20}$/.test(s));
  if (sns[0]) details["DETAILS.aircraftSN"] = sns[0];
  if (sns[1]) details["DETAILS.batterySN"] = sns[1];
  if (sns[2]) details["DETAILS.fcSN"] = sns[2];
  // aircraftName: første streng som inneholder små bokstaver eller mellomrom
  const name = stringMatches.find((s) => /[a-z ]/.test(s) && s.length < 32);
  if (name) details["DETAILS.aircraftName"] = name.trim();

  // startTime: les eventuell timestamp i siste del av blokken (LE u64 ms epoch)
  if (block.length >= 8) {
    try {
      const ts = Number(block.readBigUInt64LE(block.length - 8));
      if (ts > 1_400_000_000_000 && ts < 4_000_000_000_000) {
        details["DETAILS.startTime"] = new Date(ts).toISOString();
      }
    } catch {}
  }
  return details;
}

function haversine(
  lat1: number,
  lon1: number,
  lat2: number,
  lon2: number,
): number {
  const R = 6_371_000;
  const toRad = (d: number) => (d * Math.PI) / 180;
  const dLat = toRad(lat2 - lat1);
  const dLon = toRad(lon2 - lon1);
  const a =
    Math.sin(dLat / 2) ** 2 +
    Math.cos(toRad(lat1)) * Math.cos(toRad(lat2)) * Math.sin(dLon / 2) ** 2;
  return R * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
}

function filterFields(samples: Sample[], fields: string[]): Sample[] {
  if (!fields || fields.length === 0) return samples;
  const set = new Set(fields);
  set.add("t");
  set.add("OSD.flyTime [ms]");
  return samples.map((s) => {
    const out: Sample = {};
    for (const k of Object.keys(s)) {
      if (set.has(k)) out[k] = s[k];
    }
    return out;
  });
}
