"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true
});
exports.default = void 0;
exports.unstable_getServerSession = unstable_getServerSession;

var _core = require("../core");

var _node = require("../utils/node");

async function NextAuthHandler(req, res, options) {
  var _options$secret, _options$jwt$secret, _options$jwt, _options$trustHost, _ref, _ref2, _process$env$NEXTAUTH;

  const headers = new Headers(req.headers);
  const url = (0, _node.getURL)(req.url, headers);

  if (url instanceof Error) {
    var _options$logger$error, _options$logger;

    if (process.env.NODE_ENV !== "production") throw url;
    const errorLogger = (_options$logger$error = (_options$logger = options.logger) === null || _options$logger === void 0 ? void 0 : _options$logger.error) !== null && _options$logger$error !== void 0 ? _options$logger$error : console.error;
    errorLogger("INVALID_URL", url);
    res.status(400);
    return res.json({
      message: "There is a problem with the server configuration. Check the server logs for more information."
    });
  }

  const request = new Request(url, {
    headers,
    method: req.method,
    ...(0, _node.getBody)(req)
  });
  (_options$secret = options.secret) !== null && _options$secret !== void 0 ? _options$secret : options.secret = (_options$jwt$secret = (_options$jwt = options.jwt) === null || _options$jwt === void 0 ? void 0 : _options$jwt.secret) !== null && _options$jwt$secret !== void 0 ? _options$jwt$secret : process.env.NEXTAUTH_SECRET;
  (_options$trustHost = options.trustHost) !== null && _options$trustHost !== void 0 ? _options$trustHost : options.trustHost = !!((_ref = (_ref2 = (_process$env$NEXTAUTH = process.env.NEXTAUTH_URL) !== null && _process$env$NEXTAUTH !== void 0 ? _process$env$NEXTAUTH : process.env.AUTH_TRUST_HOST) !== null && _ref2 !== void 0 ? _ref2 : process.env.VERCEL) !== null && _ref !== void 0 ? _ref : process.env.NODE_ENV !== "production");
  const response = await (0, _core.AuthHandler)(request, options);
  res.status(response.status);
  (0, _node.setHeaders)(response.headers, res);
  return res.send(await response.text());
}

function NextAuth(...args) {
  if (args.length === 1) {
    return async (req, res) => await NextAuthHandler(req, res, args[0]);
  }

  return NextAuthHandler(args[0], args[1], args[2]);
}

var _default = NextAuth;
exports.default = _default;
let experimentalWarningShown = false;
let experimentalRSCWarningShown = false;

async function unstable_getServerSession(...args) {
  var _options, _options$secret2, _res$removeHeader, _res;

  if (!experimentalWarningShown && process.env.NODE_ENV !== "production") {
    console.warn("[next-auth][warn][EXPERIMENTAL_API]", "\n`unstable_getServerSession` is experimental and may be removed or changed in the future, as the name suggested.", `\nhttps://next-auth.js.org/configuration/nextjs#unstable_getServerSession}`, `\nhttps://next-auth.js.org/warnings#EXPERIMENTAL_API`);
    experimentalWarningShown = true;
  }

  const isRSC = args.length === 0 || args.length === 1;

  if (!experimentalRSCWarningShown && isRSC && process.env.NODE_ENV !== "production") {
    console.warn("[next-auth][warn][EXPERIMENTAL_API]", "\n`unstable_getServerSession` is used in a React Server Component.", `\nhttps://next-auth.js.org/configuration/nextjs#unstable_getServerSession}`, `\nhttps://next-auth.js.org/warnings#EXPERIMENTAL_API`);
    experimentalRSCWarningShown = true;
  }

  let req, res, options;

  if (isRSC) {
    options = Object.assign({}, args[0], {
      providers: []
    });

    const {
      headers,
      cookies
    } = require("next/headers");

    req = {
      headers: Object.fromEntries(headers()),
      cookies: Object.fromEntries(cookies().getAll().map(c => [c.name, c.value]))
    };
    res = {
      getHeader() {},

      setCookie() {},

      setHeader() {}

    };
  } else {
    req = args[0];
    res = args[1];
    options = Object.assign({}, args[2], {
      providers: []
    });
  }

  const url = (0, _node.getURL)("/api/auth/session", new Headers(req.headers));

  if (url instanceof Error) {
    var _options$logger$error2, _options$logger2;

    if (process.env.NODE_ENV !== "production") throw url;
    const errorLogger = (_options$logger$error2 = (_options$logger2 = options.logger) === null || _options$logger2 === void 0 ? void 0 : _options$logger2.error) !== null && _options$logger$error2 !== void 0 ? _options$logger$error2 : console.error;
    errorLogger("INVALID_URL", url);
    res.status(400);
    return res.json({
      message: "There is a problem with the server configuration. Check the server logs for more information."
    });
  }

  const request = new Request(url, {
    headers: new Headers(req.headers)
  });
  (_options$secret2 = (_options = options).secret) !== null && _options$secret2 !== void 0 ? _options$secret2 : _options.secret = process.env.NEXTAUTH_SECRET;
  options.trustHost = true;
  const response = await (0, _core.AuthHandler)(request, options);
  const {
    status = 200,
    headers
  } = response;
  (0, _node.setHeaders)(headers, res);
  (_res$removeHeader = (_res = res).removeHeader) === null || _res$removeHeader === void 0 ? void 0 : _res$removeHeader.call(_res, "Content-Type");
  const data = await response.json();
  if (!data || !Object.keys(data).length) return null;

  if (status === 200) {
    if (isRSC) delete data.expires;
    return data;
  }

  throw new Error(data.message);
}