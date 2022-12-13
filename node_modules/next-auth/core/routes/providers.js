"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true
});
exports.default = providers;

function providers(providers) {
  return {
    headers: {
      "Content-Type": "application/json"
    },
    body: providers.reduce((acc, {
      id,
      name,
      type,
      signinUrl,
      callbackUrl
    }) => {
      acc[id] = {
        id,
        name,
        type,
        signinUrl,
        callbackUrl
      };
      return acc;
    }, {})
  };
}