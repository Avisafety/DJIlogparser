import { extractTxtFromZip, looksLikeZip } from "./unzip.js";
import { parseDjiTxt, type InternalParseResult } from "./dji-txt.js";

export interface ParseInput {
  file: Buffer;
  fields?: string[];
}

export async function parseDjiLog(
  input: ParseInput,
): Promise<InternalParseResult> {
  let txtBuffer = input.file;
  if (looksLikeZip(txtBuffer)) {
    try {
      txtBuffer = await extractTxtFromZip(txtBuffer);
    } catch (e) {
      return {
        unsupported: true,
        reason: `failed to unzip: ${(e as Error).message}`,
      };
    }
  }
  return parseDjiTxt(txtBuffer, { fields: input.fields ?? [] });
}
