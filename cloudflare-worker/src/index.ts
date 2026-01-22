/**
 * SSControl Signaling Server
 *
 * A lightweight WebRTC signaling relay using Cloudflare Workers.
 * Handles session registration, SDP/ICE exchange, and automatic expiration.
 */

export interface Env {
  SESSIONS: KVNamespace;
  MAX_SESSION_TTL: string;
  MAX_ATTEMPTS: string;
}

interface Session {
  id: string;
  created_at: number;
  expires_at: number;

  // Host (controlled device) info
  host_offer: string;
  host_candidates: IceCandidate[];
  host_public_key?: string;
  pin_hash?: string;

  // Client (controller) info - filled after connection
  client_answer?: string;
  client_candidates?: IceCandidate[];

  status: 'waiting' | 'connecting' | 'connected' | 'expired';
  attempts: number;
}

interface IceCandidate {
  candidate: string;
  sdpMid?: string;
  sdpMLineIndex?: number;
}

// CORS headers for cross-origin requests
const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, POST, DELETE, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type',
};

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    const url = new URL(request.url);
    const path = url.pathname;

    try {
      // Route requests
      if (path === '/api/session' && request.method === 'POST') {
        return await handleCreateSession(request, env);
      }

      if (path.startsWith('/api/session/') && request.method === 'GET') {
        const sessionId = path.split('/')[3];
        return await handleGetSession(sessionId, env);
      }

      if (path.match(/^\/api\/session\/[^/]+\/answer$/) && request.method === 'POST') {
        const sessionId = path.split('/')[3];
        return await handlePostAnswer(sessionId, request, env);
      }

      if (path.match(/^\/api\/session\/[^/]+\/ice$/) && request.method === 'POST') {
        const sessionId = path.split('/')[3];
        return await handlePostIce(sessionId, request, env);
      }

      if (path.startsWith('/api/session/') && request.method === 'DELETE') {
        const sessionId = path.split('/')[3];
        return await handleDeleteSession(sessionId, env);
      }

      // Health check
      if (path === '/health') {
        return jsonResponse({ status: 'ok', timestamp: Date.now() });
      }

      return jsonResponse({ error: 'Not found' }, 404);
    } catch (error) {
      console.error('Error:', error);
      return jsonResponse({ error: 'Internal server error' }, 500);
    }
  },
};

/**
 * Create a new session (called by host/controlled device)
 */
async function handleCreateSession(request: Request, env: Env): Promise<Response> {
  const body = await request.json() as {
    session_id: string;
    offer: string;
    candidates?: IceCandidate[];
    public_key?: string;
    pin_hash?: string;
    ttl?: number;
  };

  const { session_id, offer, candidates, public_key, pin_hash, ttl } = body;

  if (!session_id || !offer) {
    return jsonResponse({ error: 'session_id and offer are required' }, 400);
  }

  const maxTtl = parseInt(env.MAX_SESSION_TTL || '300');
  const sessionTtl = Math.min(ttl || 300, maxTtl);
  const now = Date.now();

  const session: Session = {
    id: session_id,
    created_at: now,
    expires_at: now + sessionTtl * 1000,
    host_offer: offer,
    host_candidates: candidates || [],
    host_public_key: public_key,
    pin_hash,
    status: 'waiting',
    attempts: 0,
  };

  // Store in KV with TTL
  await env.SESSIONS.put(
    `session:${session_id}`,
    JSON.stringify(session),
    { expirationTtl: sessionTtl }
  );

  return jsonResponse({
    success: true,
    session_id,
    expires_at: session.expires_at,
  });
}

/**
 * Get session info (called by client/controller)
 */
async function handleGetSession(sessionId: string, env: Env): Promise<Response> {
  const data = await env.SESSIONS.get(`session:${sessionId}`);

  if (!data) {
    return jsonResponse({ error: 'Session not found or expired' }, 404);
  }

  const session: Session = JSON.parse(data);

  // Check if expired
  if (Date.now() > session.expires_at) {
    await env.SESSIONS.delete(`session:${sessionId}`);
    return jsonResponse({ error: 'Session expired' }, 410);
  }

  // Update attempts
  session.attempts++;
  const maxAttempts = parseInt(env.MAX_ATTEMPTS || '3');

  if (session.attempts > maxAttempts) {
    await env.SESSIONS.delete(`session:${sessionId}`);
    return jsonResponse({ error: 'Too many attempts' }, 429);
  }

  // Save updated session
  const remainingTtl = Math.ceil((session.expires_at - Date.now()) / 1000);
  await env.SESSIONS.put(
    `session:${sessionId}`,
    JSON.stringify(session),
    { expirationTtl: remainingTtl }
  );

  // Return public info only
  return jsonResponse({
    offer: session.host_offer,
    candidates: session.host_candidates,
    public_key: session.host_public_key,
    expires_at: session.expires_at,
    status: session.status,
    // Also return answer if available (for host polling)
    answer: session.client_answer,
    client_candidates: session.client_candidates,
  });
}

/**
 * Post answer from client to host
 */
async function handlePostAnswer(sessionId: string, request: Request, env: Env): Promise<Response> {
  const data = await env.SESSIONS.get(`session:${sessionId}`);

  if (!data) {
    return jsonResponse({ error: 'Session not found or expired' }, 404);
  }

  const session: Session = JSON.parse(data);

  if (Date.now() > session.expires_at) {
    await env.SESSIONS.delete(`session:${sessionId}`);
    return jsonResponse({ error: 'Session expired' }, 410);
  }

  const body = await request.json() as {
    answer: string;
    candidates?: IceCandidate[];
  };

  if (!body.answer) {
    return jsonResponse({ error: 'answer is required' }, 400);
  }

  // Update session with client answer
  session.client_answer = body.answer;
  session.client_candidates = body.candidates || [];
  session.status = 'connecting';

  const remainingTtl = Math.ceil((session.expires_at - Date.now()) / 1000);
  await env.SESSIONS.put(
    `session:${sessionId}`,
    JSON.stringify(session),
    { expirationTtl: remainingTtl }
  );

  return jsonResponse({ success: true });
}

/**
 * Post additional ICE candidates
 */
async function handlePostIce(sessionId: string, request: Request, env: Env): Promise<Response> {
  const data = await env.SESSIONS.get(`session:${sessionId}`);

  if (!data) {
    return jsonResponse({ error: 'Session not found or expired' }, 404);
  }

  const session: Session = JSON.parse(data);

  const body = await request.json() as {
    role: 'host' | 'client';
    candidate: IceCandidate;
  };

  if (!body.candidate) {
    return jsonResponse({ error: 'candidate is required' }, 400);
  }

  // Add candidate to appropriate list
  if (body.role === 'host') {
    session.host_candidates.push(body.candidate);
  } else {
    if (!session.client_candidates) {
      session.client_candidates = [];
    }
    session.client_candidates.push(body.candidate);
  }

  const remainingTtl = Math.ceil((session.expires_at - Date.now()) / 1000);
  await env.SESSIONS.put(
    `session:${sessionId}`,
    JSON.stringify(session),
    { expirationTtl: remainingTtl }
  );

  return jsonResponse({ success: true });
}

/**
 * Delete a session
 */
async function handleDeleteSession(sessionId: string, env: Env): Promise<Response> {
  await env.SESSIONS.delete(`session:${sessionId}`);
  return jsonResponse({ success: true });
}

/**
 * Helper to create JSON responses with CORS headers
 */
function jsonResponse(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      'Content-Type': 'application/json',
      ...corsHeaders,
    },
  });
}
