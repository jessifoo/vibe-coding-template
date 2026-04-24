/**
 * Schemas aligned with the Rust Axum API (`backend/src/models/`).
 * Use at API boundaries so client and server stay type-consistent at runtime.
 */
import { z } from 'zod';

/** Lowercase provider names as serialized by Rust `#[serde(rename_all = "lowercase")]`. */
export const LlmProviderSchema = z.enum(['openai', 'anthropic']);
export type LlmProvider = z.infer<typeof LlmProviderSchema>;

export const LlmUsageSchema = z.object({
  prompt_tokens: z.number().int().nonnegative(),
  completion_tokens: z.number().int().nonnegative().nullable().optional(),
  total_tokens: z.number().int().nonnegative(),
});

export type LlmUsage = z.infer<typeof LlmUsageSchema>;

export const TextGenerationRequestSchema = z.object({
  prompt: z.string().min(1),
  model: z.string().default('gpt-4o-mini'),
  max_tokens: z.number().int().min(1).max(4000).default(500),
  temperature: z.number().min(0).max(2).default(0.7),
  provider: LlmProviderSchema.default('openai'),
});

export type TextGenerationRequest = z.infer<typeof TextGenerationRequestSchema>;

export const TextGenerationResponseSchema = z.object({
  text: z.string(),
  model: z.string(),
  usage: LlmUsageSchema,
});

export type TextGenerationResponse = z.infer<typeof TextGenerationResponseSchema>;

export const EmbeddingRequestSchema = z.object({
  text: z.string().min(1),
  model: z.string().default('text-embedding-3-small'),
  provider: LlmProviderSchema.default('openai'),
});

export type EmbeddingRequest = z.infer<typeof EmbeddingRequestSchema>;

export const EmbeddingResponseSchema = z.object({
  embedding: z.array(z.number()),
  model: z.string(),
  usage: LlmUsageSchema,
});

export type EmbeddingResponse = z.infer<typeof EmbeddingResponseSchema>;

/** Axum `AppError` JSON body: `{ "error": "..." }` */
export const ApiErrorBodySchema = z.object({
  error: z.string(),
});

export type ApiErrorBody = z.infer<typeof ApiErrorBodySchema>;
