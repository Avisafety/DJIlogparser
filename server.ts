import Fastify from "fastify";
import multipart from "@fastify/multipart";
import { parseDjiLog } from "./parser/index.js";

const PORT = Number(process.env.PORT ?? 8080);
const TOKEN = process.env.AVISAFE_PARSER_TOKEN;

const app = Fastify({
  logger: { level: process.env.LOG_LEVEL ?? "info" },
  bodyLimit: 200 * 1024 * 1024, // 200 MB
});

await app.register(multipart, {
  limits: { fileSize: 200 * 1024 * 1024, files: 1 },
});

app.get("/health", async () => ({ ok: true, version: "0.1.0" }));

app.post("/parse", async (req, reply) => {
  // Auth
  if (TOKEN) {
    const auth = req.headers.authorization;
    if (auth !== `Bearer ${TOKEN}`) {
      return reply.code(401).send({ error: "unauthorized" });
    }
  }

  let fileBuffer: Buffer | undefined;
  let fields: string[] = [];

  const parts = req.parts();
  for await (const part of parts) {
    if (part.type === "file" && part.fieldname === "file") {
      const chunks: Buffer[] = [];
      for await (const chunk of part.file) chunks.push(chunk as Buffer);
      fileBuffer = Buffer.concat(chunks);
    } else if (part.type === "field" && part.fieldname === "fields") {
      const v = String(part.value ?? "");
      fields = v
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
    }
  }

  if (!fileBuffer) {
    return reply.code(400).send({ error: "missing file field" });
  }

  req.log.info(
    { bytes: fileBuffer.length, fields: fields.length },
    "parsing dji log",
  );

  const out = await parseDjiLog({ file: fileBuffer, fields });
  if (out.unsupported) {
    req.log.warn({ reason: out.reason }, "unsupported log");
    return reply
      .code(422)
      .send({ unsupported: true, reason: out.reason ?? "unsupported" });
  }
  return reply.send(out.result);
});

app.setErrorHandler((err, _req, reply) => {
  app.log.error(err);
  reply.code(500).send({ error: "internal", message: err.message });
});

await app.listen({ host: "0.0.0.0", port: PORT });
app.log.info(`avisafe-djilog-parser listening on :${PORT}`);
