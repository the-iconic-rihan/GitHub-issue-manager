"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true
});
exports.toInternalRequest = toInternalRequest;
exports.toResponse = toResponse;

var _cookie = require("cookie");

var _errors = require("../core/errors");

const decoder = new TextDecoder();

async function streamToString(stream) {
  const chunks = [];
  return await new Promise((resolve, reject) => {
    stream.on("data", chunk => chunks.push(Buffer.from(chunk)));
    stream.on("error", err => reject(err));
    stream.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
  });
}

async function readJSONBody(body) {
  try {
    if ("getReader" in body) {
      const reader = body.getReader();
      const bytes = [];

      while (true) {
        const {
          value,
          done
        } = await reader.read();
        if (done) break;
        bytes.push(...value);
      }

      const b = new Uint8Array(bytes);
      return JSON.parse(decoder.decode(b));
    }

    if (typeof Buffer !== "undefined" && Buffer.isBuffer(body)) {
      return JSON.parse(body.toString("utf8"));
    }

    return JSON.parse(await streamToString(body));
  } catch (e) {
    console.error(e);
  }
}

const actions = ["providers", "session", "csrf", "signin", "signout", "callback", "verify-request", "error", "_log"];

async function toInternalRequest(req) {
  try {
    var _req$method, _parseCookie, _req$headers$get, _url$searchParams$get;

    const url = new URL(req.url.replace(/\/$/, ""));
    const {
      pathname
    } = url;
    const action = actions.find(a => pathname.includes(a));

    if (!action) {
      throw new _errors.UnknownAction("Cannot detect action.");
    }

    const providerIdOrAction = pathname.split("/").pop();
    let providerId;

    if (providerIdOrAction && !action.includes(providerIdOrAction) && ["signin", "callback"].includes(action)) {
      providerId = providerIdOrAction;
    }

    return {
      url,
      action,
      providerId,
      method: (_req$method = req.method) !== null && _req$method !== void 0 ? _req$method : "GET",
      headers: Object.fromEntries(req.headers),
      body: req.body ? await readJSONBody(req.body) : undefined,
      cookies: (_parseCookie = (0, _cookie.parse)((_req$headers$get = req.headers.get("cookie")) !== null && _req$headers$get !== void 0 ? _req$headers$get : "")) !== null && _parseCookie !== void 0 ? _parseCookie : {},
      error: (_url$searchParams$get = url.searchParams.get("error")) !== null && _url$searchParams$get !== void 0 ? _url$searchParams$get : undefined,
      query: Object.fromEntries(url.searchParams)
    };
  } catch (error) {
    return error;
  }
}

function toResponse(res) {
  var _res$cookies, _res$status;

  const headers = new Headers(res.headers);
  (_res$cookies = res.cookies) === null || _res$cookies === void 0 ? void 0 : _res$cookies.forEach(cookie => {
    const {
      name,
      value,
      options
    } = cookie;
    const cookieHeader = (0, _cookie.serialize)(name, value, options);

    if (headers.has("Set-Cookie")) {
      headers.append("Set-Cookie", cookieHeader);
    } else {
      headers.set("Set-Cookie", cookieHeader);
    }
  });
  const body = headers.get("content-type") === "application/json" ? JSON.stringify(res.body) : res.body;
  const response = new Response(body, {
    headers,
    status: res.redirect ? 302 : (_res$status = res.status) !== null && _res$status !== void 0 ? _res$status : 200
  });

  if (res.redirect) {
    response.headers.set("Location", res.redirect);
  }

  return response;
}