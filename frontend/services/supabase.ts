import { createBrowserClient } from '@supabase/ssr';
import type {
  AuthChangeEvent,
  Session,
  User,
} from '@supabase/supabase-js';

// `@supabase/ssr`'s createBrowserClient handles cookie-based session storage
// automatically and is the supported successor to the now-deprecated
// `@supabase/auth-helpers-nextjs`. The browser auth API surface
// (`supabase.auth.signInWith*`, `getSession`, `onAuthStateChange`, etc.) is
// unchanged because @supabase/ssr re-exports the same SupabaseClient type
// from @supabase/supabase-js.
const supabaseUrl = process.env.NEXT_PUBLIC_SUPABASE_URL || 'https://placeholder.supabase.co';
const supabaseAnonKey = process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY || 'placeholder-anon-key';

if (!process.env.NEXT_PUBLIC_SUPABASE_URL || !process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY) {
  // console.warn is whitelisted by ESLint config; surfaces a misconfiguration
  // loudly at module load so local-dev / first-time-setup hits an actionable
  // signal rather than an opaque auth failure later.
  console.warn('Missing Supabase environment variables. Authentication might not work correctly.');
}

export const supabase = createBrowserClient(supabaseUrl, supabaseAnonKey);

// Authentication helpers
export async function signInWithGoogle() {
  return supabase.auth.signInWithOAuth({
    provider: 'google',
    options: {
      redirectTo: `${window.location.origin}/auth/callback`,
    },
  });
}

export async function signInWithLinkedIn() {
  return supabase.auth.signInWithOAuth({
    provider: 'linkedin',
    options: {
      redirectTo: `${window.location.origin}/auth/callback`,
    },
  });
}

// Email password authentication
export async function signInWithEmail(email: string, password: string) {
  return supabase.auth.signInWithPassword({
    email,
    password,
  });
}

export async function signUpWithEmail(email: string, password: string) {
  return supabase.auth.signUp({
    email,
    password,
    options: {
      emailRedirectTo: `${window.location.origin}/auth/callback`,
    },
  });
}

export async function resetPassword(email: string) {
  return supabase.auth.resetPasswordForEmail(email, {
    redirectTo: `${window.location.origin}/auth/reset-password`,
  });
}

export async function signOut() {
  return supabase.auth.signOut();
}

export async function getCurrentUser(): Promise<User | null> {
  const {
    data: { user },
  } = await supabase.auth.getUser();
  return user;
}

// Session management
export function onAuthStateChange(
  callback: (event: AuthChangeEvent, session: Session | null) => void
) {
  return supabase.auth.onAuthStateChange(callback);
}