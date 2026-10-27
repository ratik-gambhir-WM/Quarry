// Run with the reference checkout's tsx loader; all outputs stay beside this script.
import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
const referenceServer = path.resolve(process.argv[2]);
const referenceRoot = path.dirname(referenceServer);
const load = (base, file) => import(pathToFileURL(path.join(base, file)).href);
const { parseResponse } = await load(referenceRoot, 'node_modules/openai/lib/ResponsesParser.mjs');
const { OpenAISlideClassifier } = await load(referenceServer, 'src/integrations/OpenAISlideClassifier.ts');
const { OpenAISlideEmbedder } = await load(referenceServer, 'src/integrations/OpenAISlideEmbedder.ts');
const { VALID_METADATA } = await load(referenceServer, 'test/slideTestFixtures.ts');
const text = value => ({ type: 'output_text', text: typeof value === 'string' ? value : JSON.stringify(value) });
const message = (...content) => ({ type: 'message', content });
const complete = (...output) => ({ status: 'completed', output });
const refusal = { type: 'refusal', refusal: 'declined' };
const fixture = { classification: [], embedding: [] };
for (const [name, response] of Object.entries({
  valid: complete(message(text(VALID_METADATA))),
  validThenInvalid: complete(message(text(VALID_METADATA), text({}))),
  invalidThenRefusal: complete(message(text({}), refusal)),
  refusalThenMalformed: complete(message(refusal, text('not json'))),
  invalidInSecondMessage: complete(message(text(VALID_METADATA)), message(text({}))),
  firstOutputWins: complete(message(text(VALID_METADATA), text({ ...VALID_METADATA, slide_type: 'other' }))),
  incompleteMalformed: { status: 'incomplete', output: [message(text('not json'))] },
  incompleteRefusalMalformed: { status: 'incomplete', output: [message(refusal, text('not json'))] },
  missingStatusInvalid: { output: [message(text({}))] },
  missingStatusValid: { output: [message(text(VALID_METADATA))] },
  missingOutput: { status: 'completed' },
  missingContent: complete({ type: 'message' }),
  ignoredOutputParsed: { ...complete(), output_parsed: VALID_METADATA },
  nullText: complete(message({ type: 'output_text', text: null })),
  missingText: complete(message({ type: 'output_text' })),
  nullOutput: complete(null),
  nullContent: complete(message(null)),
  empty: complete(),
  refusal: complete(message(refusal)),
  unknownOutput: complete({ type: 'function_call', name: 'ignored', arguments: 'not json' }),
})) {
  const classifier = new OpenAISlideClassifier({
    client: { responses: { parse: async (body, options) => {
      fixture.classifierRequest = body;
      fixture.classifierRequestOptions = options;
      return parseResponse(response, body);
    } } },
    maxOutputTokens: 900, model: 'configured-classifier', timeoutMs: 1234,
  });
  const entry = { name, response };
  try {
    entry.result = await classifier.classify({ digest: 'bounded digest', preview: { bytes: Buffer.from('png'), contentType: 'image/png' } });
  } catch (error) {
    entry.error = { code: error.code, retryable: error.retryable };
  }
  fixture.classification.push(entry);
}
for (const [name, response] of Object.entries({
  missingData: {}, nullData: { data: null }, objectData: { data: {} },
  emptyData: { data: [] }, nullEntry: { data: [null] }, missingVector: { data: [{}] },
  stringVector: { data: [{ embedding: 'x' }] }, zeroNorm: { data: [{ embedding: [0, 0] }] },
  wrongDimensions: { data: [{ embedding: [1] }] }, wrongTypes: { data: [{ embedding: [1, '2'] }] },
  multiple: { data: [{ embedding: [1, 0] }, { embedding: [0, 1] }] },
  valid: { data: [{ embedding: [1, 0] }] },
})) {
  const embedder = new OpenAISlideEmbedder({
    client: { embeddings: { create: async (body, options) => {
      fixture.embeddingRequest = body;
      fixture.embeddingRequestOptions = options;
      return response;
    } } },
    dimensions: 2, maxInputBytes: 100, model: 'configured-embedding', timeoutMs: 200,
  });
  const entry = { name, response };
  try { entry.result = await embedder.embed('canonical document'); }
  catch (error) { entry.error = { code: error.code, retryable: error.retryable }; }
  fixture.embedding.push(entry);
}
await fs.writeFile(new URL('reference.json', import.meta.url), `${JSON.stringify(fixture, null, 2)}\n`);
