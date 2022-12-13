"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true
});
exports.getBody = getBody;
exports.getURL = getURL;
exports.setCookie = setCookie;
exports.setHeaders = setHeaders;

function setCookie(res, value) {
  var _res$getHeader;

  let setCookieHeader = (_res$getHeader = res.getHeader("Set-Cookie")) !== null && _res$getHeader !== void 0 ? _res$getHeader : [];

  if (!Array.isArray(setCookieHeader)) {
    setCookieHeader = [setCookieHeader];
  }

  setCookieHeader.push(value);
  res.setHeader("Set-Cookie", setCookieHeader);
}

function getBody(req) {
  if (!("body" in req) || !req.body || req.method !== "POST") {
    return;
  }

  if (req.body instanceof ReadableStream) {
    return {
      body: req.body
    };
  }

  return {
    body: JSON.stringify(req.body)
  };
}

function getURL(url, headers) {
  try {
    var _headers$get, _headers$get2;

    if (!url) throw new Error("Missing url");

    if (process.env.NEXTAUTH_URL) {
      const base = new URL(process.env.NEXTAUTH_URL);

      if (!["http:", "https:"].includes(base.protocol)) {
        throw new Error("Invalid protocol");
      }

      const hasCustomPath = base.pathname !== "/";

      if (hasCustomPath) {
        const apiAuthRe = /\/api\/auth\/?$/;
        const basePathname = base.pathname.match(apiAuthRe) ? base.pathname.replace(apiAuthRe, "") : base.pathname;
        return new URL(basePathname.replace(/\/$/, "") + url, base.origin);
      }

      return new URL(url, base);
    }

    const proto = (_headers$get = headers.get("x-forwarded-proto")) !== null && _headers$get !== void 0 ? _headers$get : process.env.NODE_ENV !== "production" ? "http" : "https";
    const host = (_headers$get2 = headers.get("x-forwarded-host")) !== null && _headers$get2 !== void 0 ? _headers$get2 : headers.get("host");
    if (!["http", "https"].includes(proto)) throw new Error("Invalid protocol");
    const origin = `${proto}://${host}`;
    if (!host) throw new Error("Missing host");
    return new URL(url, origin);
  } catch (error) {
    return error;
  }
}

function getSetCookies(cookiesString) {
  if (typeof cookiesString !== "string") {
    return [];
  }

  const cookiesStrings = [];
  let pos = 0;
  let start;
  let ch;
  let lastComma;
  let nextStart;
  let cookiesSeparatorFound;

  function skipWhitespace() {
    while (pos < cookiesString.length && /\s/.test(cookiesString.charAt(pos))) {
      pos += 1;
    }

    return pos < cookiesString.length;
  }

  function notSpecialChar() {
    ch = cookiesString.charAt(pos);
    return ch !== "=" && ch !== ";" && ch !== ",";
  }

  while (pos < cookiesString.length) {
    start = pos;
    cookiesSeparatorFound = false;

    while (skipWhitespace()) {
      ch = cookiesString.charAt(pos);

      if (ch === ",") {
        lastComma = pos;
        pos += 1;
        skipWhitespace();
        nextStart = pos;

        while (pos < cookiesString.length && notSpecialChar()) {
          pos += 1;
        }

        if (pos < cookiesString.length && cookiesString.charAt(pos) === "=") {
          cookiesSeparatorFound = true;
          pos = nextStart;
          cookiesStrings.push(cookiesString.substring(start, lastComma));
          start = pos;
        } else {
          pos = lastComma + 1;
        }
      } else {
        pos += 1;
      }
    }

    if (!cookiesSeparatorFound || pos >= cookiesString.length) {
      cookiesStrings.push(cookiesString.substring(start, cookiesString.length));
    }
  }

  return cookiesStrings;
}

function setHeaders(headers, res) {
  for (const [key, val] of headers.entries()) {
    let value = val;

    if (key === "set-cookie") {
      const cookies = getSetCookies(value);
      let original = res.getHeader("set-cookie");
      original = Array.isArray(original) ? original : [original];
      value = original.concat(cookies).filter(Boolean);
    }

    res.setHeader(key, value);
  }
}