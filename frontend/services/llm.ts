import { supabase } from './supabase';
import {
  TextGenerationRequestSchema,
  TextGenerationResponseSchema,
  EmbeddingRequestSchema,
  EmbeddingResponseSchema,
  ApiErrorBodySchema,
  type TextGenerationRequest,
  type TextGenerationResponse,
  type EmbeddingRequest,
  type EmbeddingResponse,
} from '@/lib/api-types';

export type {
  TextGenerationRequest,
  TextGenerationResponse,
  EmbeddingRequest,
  EmbeddingResponse,
} from '@/lib/api-types';

const getApiUrl = (): string =>
  typeof window !== 'undefined'
    ? process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8000'
    : 'http://backend:8000';

async function getAuthToken(): Promise<string> {
  const {
    data: { session },
  } = await supabase.auth.getSession();
  const token = session?.access_token;
  if (!token) {
    throw new Error('Authentication required');
  }
  return token;
}

function parseJsonErrorMessage(body: unknown, fallback: string): string {
  const parsed = ApiErrorBodySchema.safeParse(body);
  if (parsed.success) {
    return parsed.data.error;
  }
  if (body !== null && typeof body === 'object' && 'detail' in body) {
    const d = (body as { detail?: unknown }).detail;
    if (typeof d === 'string') {
      return d;
    }
  }
  return fallback;
}

async function apiRequestJson<T>(
  endpoint: string,
  method: string,
  body: unknown,
  parse: (data: unknown) => T
): Promise<T> {
  const token = await getAuthToken();
  const API_URL = getApiUrl();
  const url = `${API_URL}${endpoint}`;

  const response = await fetch(url, {
    method,
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
      Authorization: `Bearer ${token}`,
    },
    mode: 'cors',
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });

  if (!response.ok) {
    let errBody: unknown;
    try {
      errBody = await response.json();
    } catch {
      errBody = undefined;
    }
    const message = parseJsonErrorMessage(
      errBody,
      `API request failed with status ${response.status}`
    );
    throw new Error(message);
  }

  const raw: unknown = await response.json();
  return parse(raw);
}

export async function generateText(
  request: TextGenerationRequest
): Promise<TextGenerationResponse> {
  const payload = TextGenerationRequestSchema.parse(request);
  return apiRequestJson(
    '/api/llm/generate',
    'POST',
    payload,
    (data) => TextGenerationResponseSchema.parse(data)
  );
}

export async function createEmbedding(
  request: EmbeddingRequest
): Promise<EmbeddingResponse> {
  const payload = EmbeddingRequestSchema.parse(request);
  return apiRequestJson(
    '/api/llm/embedding',
    'POST',
    payload,
    (data) => EmbeddingResponseSchema.parse(data)
  );
}
