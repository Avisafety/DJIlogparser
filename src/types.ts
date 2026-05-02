export type SampleValue = number | string | undefined;
export type Sample = Record<string, SampleValue>;
export type Details = Record<string, SampleValue>;

export interface ParseResult {
  details: Details;
  samples: Sample[];
  summary: {
    maxAltitude: number;
    maxHSpeed: number;
    maxVSpeed: number;
    maxDistance: number;
    totalDistance: number;
    totalFlightTime: number;
    sampleCount: number;
  };
}
