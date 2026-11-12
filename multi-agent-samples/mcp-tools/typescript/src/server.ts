import { McpServer } from '@modelcontextprotocol/server';
import { serveStdio } from '@modelcontextprotocol/server/stdio';
import * as z from 'zod/v4';

// Every tool answers with an object that has one field, "result".
// It is sent twice: as structuredContent, and as the text of one text block.
function answer(result: number | number[]) {
  const structuredContent = { result };
  return { content: [{ type: 'text' as const, text: JSON.stringify(structuredContent) }], structuredContent };
}

// A wrong input is an error of the tool: the text, with isError set.
function failure(message: string) {
  return { content: [{ type: 'text' as const, text: message }], isError: true };
}

// The name and the version are what this server tells Metagente about itself.
function createServer(): McpServer {
  const server = new McpServer({ name: 'calc', version: '1.0.0' });

  server.registerTool(
    'mean',
    {
      description: 'Average of a list of numbers.',
      inputSchema: z.object({ values: z.array(z.number()) }),
    },
    async ({ values }) => {
      if (values.length === 0) return failure('values must not be empty');
      return answer(values.reduce((sum, v) => sum + v, 0) / values.length);
    },
  );

  server.registerTool(
    'dot',
    {
      description: 'Dot product of two lists of numbers of the same size.',
      inputSchema: z.object({ a: z.array(z.number()), b: z.array(z.number()) }),
    },
    async ({ a, b }) => {
      if (a.length !== b.length) return failure('vectors must have the same size');
      if (a.length === 0) return failure('values must not be empty');
      return answer(a.reduce((sum, x, i) => sum + x * b[i], 0));
    },
  );

  server.registerTool(
    'scale',
    {
      description: 'Every number of the list multiplied by the factor.',
      inputSchema: z.object({ values: z.array(z.number()), factor: z.number() }),
    },
    async ({ values, factor }) => {
      if (values.length === 0) return failure('values must not be empty');
      return answer(values.map((v) => v * factor));
    },
  );

  return server;
}

// Talk to Metagente through standard input and output.
// Never print anything else to standard output (console.log): it would break the conversation.
// For messages to yourself use console.error.
void serveStdio(createServer);
