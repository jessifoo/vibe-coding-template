import { createServerClient, type CookieOptions } from '@supabase/ssr';
import { cookies } from 'next/headers';
import { NextRequest, NextResponse } from 'next/server';

/**
 * OAuth callback handler.
 *
 * Exchanges the `?code=` PKCE code returned by the OAuth provider for a
 * Supabase session, then redirects to the dashboard. Replaces the
 * deprecated `@supabase/auth-helpers-nextjs::createRouteHandlerClient`
 * pattern with `@supabase/ssr::createServerClient`, which is the supported
 * route for cookie-based session storage in the App Router.
 *
 * Next 15 made `cookies()` async; we `await` it once and pass `getAll` /
 * `setAll` adapters into `createServerClient` so the Supabase SDK can
 * mutate the response cookies through Next's cookie store.
 */
export async function GET(request: NextRequest): Promise<NextResponse> {
  const requestUrl = new URL(request.url);
  const code = requestUrl.searchParams.get('code');
  const redirectTo = `${requestUrl.origin}/dashboard`;

  if (!code) {
    return NextResponse.redirect(redirectTo);
  }

  const supabaseUrl = process.env.NEXT_PUBLIC_SUPABASE_URL;
  const supabaseAnonKey = process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY;
  if (!supabaseUrl || !supabaseAnonKey) {
    // Without credentials we can't exchange the code; redirect anyway so the
    // user lands somewhere instead of a hung route. The browser-side client
    // (services/supabase.ts) already warns on the same missing env vars at
    // module load, so this misconfiguration is surfaced loudly elsewhere.
    console.warn(
      'OAuth callback: NEXT_PUBLIC_SUPABASE_URL / NEXT_PUBLIC_SUPABASE_ANON_KEY missing; skipping code exchange.',
    );
    return NextResponse.redirect(redirectTo);
  }

  const cookieStore = await cookies();
  const supabase = createServerClient(supabaseUrl, supabaseAnonKey, {
    cookies: {
      getAll(): ReturnType<typeof cookieStore.getAll> {
        return cookieStore.getAll();
      },
      setAll(cookiesToSet: { name: string; value: string; options: CookieOptions }[]): void {
        for (const { name, value, options } of cookiesToSet) {
          cookieStore.set(name, value, options);
        }
      },
    },
  });

  const { error } = await supabase.auth.exchangeCodeForSession(code);
  if (error) {
    // Log with structured context but still redirect — surfacing the auth
    // failure visually is the LoginForm's job; the callback should never
    // leave the user stuck on /auth/callback.
    console.error('OAuth code exchange failed:', error.message);
  }

  return NextResponse.redirect(redirectTo);
}