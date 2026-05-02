import unzipper from "unzipper";

/**
 * Pakker ut DJI .zip og returnerer .txt-innholdet som Buffer.
 * DJI .zip inneholder typisk én .txt-fil + evt. metadata.
 */
export async function extractTxtFromZip(buffer: Buffer): Promise<Buffer> {
  const directory = await unzipper.Open.buffer(buffer);
  const txtFile = directory.files.find(
    (f) => f.path.toLowerCase().endsWith(".txt") && f.type === "File",
  );
  if (!txtFile) {
    throw new Error("No .txt file found inside DJI .zip");
  }
  return await txtFile.buffer();
}

export function looksLikeZip(buffer: Buffer): boolean {
  // PK\x03\x04
  return (
    buffer.length > 4 &&
    buffer[0] === 0x50 &&
    buffer[1] === 0x4b &&
    buffer[2] === 0x03 &&
    buffer[3] === 0x04
  );
}
