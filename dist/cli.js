#!/usr/bin/env node
import { createRequire as __cR } from 'module';
const require = __cR(import.meta.url);
var __create = Object.create;
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __getProtoOf = Object.getPrototypeOf;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
  get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
}) : x)(function(x) {
  if (typeof require !== "undefined") return require.apply(this, arguments);
  throw Error('Dynamic require of "' + x + '" is not supported');
});
var __commonJS = (cb, mod) => function __require2() {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
  // If the importer is in node compatibility mode or this is not an ESM
  // file that has been converted to a CommonJS file using a Babel-
  // compatible transform (i.e. "__esModule" has not been set), then set
  // "default" to the CommonJS "module.exports" for node compatibility.
  isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
  mod
));

// node_modules/async-lock/lib/index.js
var require_lib = __commonJS({
  "node_modules/async-lock/lib/index.js"(exports, module) {
    "use strict";
    var AsyncLock = function(opts) {
      opts = opts || {};
      this.Promise = opts.Promise || Promise;
      this.queues = /* @__PURE__ */ Object.create(null);
      this.domainReentrant = opts.domainReentrant || false;
      if (this.domainReentrant) {
        if (typeof process === "undefined" || typeof process.domain === "undefined") {
          throw new Error(
            "Domain-reentrant locks require `process.domain` to exist. Please flip `opts.domainReentrant = false`, use a NodeJS version that still implements Domain, or install a browser polyfill."
          );
        }
        this.domains = /* @__PURE__ */ Object.create(null);
      }
      this.timeout = opts.timeout || AsyncLock.DEFAULT_TIMEOUT;
      this.maxOccupationTime = opts.maxOccupationTime || AsyncLock.DEFAULT_MAX_OCCUPATION_TIME;
      this.maxExecutionTime = opts.maxExecutionTime || AsyncLock.DEFAULT_MAX_EXECUTION_TIME;
      if (opts.maxPending === Infinity || Number.isInteger(opts.maxPending) && opts.maxPending >= 0) {
        this.maxPending = opts.maxPending;
      } else {
        this.maxPending = AsyncLock.DEFAULT_MAX_PENDING;
      }
    };
    AsyncLock.DEFAULT_TIMEOUT = 0;
    AsyncLock.DEFAULT_MAX_OCCUPATION_TIME = 0;
    AsyncLock.DEFAULT_MAX_EXECUTION_TIME = 0;
    AsyncLock.DEFAULT_MAX_PENDING = 1e3;
    AsyncLock.prototype.acquire = function(key, fn, cb, opts) {
      if (Array.isArray(key)) {
        return this._acquireBatch(key, fn, cb, opts);
      }
      if (typeof fn !== "function") {
        throw new Error("You must pass a function to execute");
      }
      var deferredResolve = null;
      var deferredReject = null;
      var deferred = null;
      if (typeof cb !== "function") {
        opts = cb;
        cb = null;
        deferred = new this.Promise(function(resolve, reject) {
          deferredResolve = resolve;
          deferredReject = reject;
        });
      }
      opts = opts || {};
      var resolved = false;
      var timer = null;
      var occupationTimer = null;
      var executionTimer = null;
      var self = this;
      var done = function(locked, err, ret) {
        if (occupationTimer) {
          clearTimeout(occupationTimer);
          occupationTimer = null;
        }
        if (executionTimer) {
          clearTimeout(executionTimer);
          executionTimer = null;
        }
        if (locked) {
          if (!!self.queues[key] && self.queues[key].length === 0) {
            delete self.queues[key];
          }
          if (self.domainReentrant) {
            delete self.domains[key];
          }
        }
        if (!resolved) {
          if (!deferred) {
            if (typeof cb === "function") {
              cb(err, ret);
            }
          } else {
            if (err) {
              deferredReject(err);
            } else {
              deferredResolve(ret);
            }
          }
          resolved = true;
        }
        if (locked) {
          if (!!self.queues[key] && self.queues[key].length > 0) {
            self.queues[key].shift()();
          }
        }
      };
      var exec = function(locked) {
        if (resolved) {
          return done(locked);
        }
        if (timer) {
          clearTimeout(timer);
          timer = null;
        }
        if (self.domainReentrant && locked) {
          self.domains[key] = process.domain;
        }
        var maxExecutionTime = opts.maxExecutionTime || self.maxExecutionTime;
        if (maxExecutionTime) {
          executionTimer = setTimeout(function() {
            if (!!self.queues[key]) {
              done(locked, new Error("Maximum execution time is exceeded " + key));
            }
          }, maxExecutionTime);
        }
        if (fn.length === 1) {
          var called = false;
          try {
            fn(function(err, ret) {
              if (!called) {
                called = true;
                done(locked, err, ret);
              }
            });
          } catch (err) {
            if (!called) {
              called = true;
              done(locked, err);
            }
          }
        } else {
          self._promiseTry(function() {
            return fn();
          }).then(function(ret) {
            done(locked, void 0, ret);
          }, function(error) {
            done(locked, error);
          });
        }
      };
      if (self.domainReentrant && !!process.domain) {
        exec = process.domain.bind(exec);
      }
      var maxPending = opts.maxPending || self.maxPending;
      if (!self.queues[key]) {
        self.queues[key] = [];
        exec(true);
      } else if (self.domainReentrant && !!process.domain && process.domain === self.domains[key]) {
        exec(false);
      } else if (self.queues[key].length >= maxPending) {
        done(false, new Error("Too many pending tasks in queue " + key));
      } else {
        var taskFn = function() {
          exec(true);
        };
        if (opts.skipQueue) {
          self.queues[key].unshift(taskFn);
        } else {
          self.queues[key].push(taskFn);
        }
        var timeout = opts.timeout || self.timeout;
        if (timeout) {
          timer = setTimeout(function() {
            timer = null;
            done(false, new Error("async-lock timed out in queue " + key));
          }, timeout);
        }
      }
      var maxOccupationTime = opts.maxOccupationTime || self.maxOccupationTime;
      if (maxOccupationTime) {
        occupationTimer = setTimeout(function() {
          if (!!self.queues[key]) {
            done(false, new Error("Maximum occupation time is exceeded in queue " + key));
          }
        }, maxOccupationTime);
      }
      if (deferred) {
        return deferred;
      }
    };
    AsyncLock.prototype._acquireBatch = function(keys, fn, cb, opts) {
      if (typeof cb !== "function") {
        opts = cb;
        cb = null;
      }
      var self = this;
      var getFn = function(key, fn2) {
        return function(cb2) {
          self.acquire(key, fn2, cb2, opts);
        };
      };
      var fnx = keys.reduceRight(function(prev, key) {
        return getFn(key, prev);
      }, fn);
      if (typeof cb === "function") {
        fnx(cb);
      } else {
        return new this.Promise(function(resolve, reject) {
          if (fnx.length === 1) {
            fnx(function(err, ret) {
              if (err) {
                reject(err);
              } else {
                resolve(ret);
              }
            });
          } else {
            resolve(fnx());
          }
        });
      }
    };
    AsyncLock.prototype.isBusy = function(key) {
      if (!key) {
        return Object.keys(this.queues).length > 0;
      } else {
        return !!this.queues[key];
      }
    };
    AsyncLock.prototype._promiseTry = function(fn) {
      try {
        return this.Promise.resolve(fn());
      } catch (e) {
        return this.Promise.reject(e);
      }
    };
    module.exports = AsyncLock;
  }
});

// node_modules/async-lock/index.js
var require_async_lock = __commonJS({
  "node_modules/async-lock/index.js"(exports, module) {
    "use strict";
    module.exports = require_lib();
  }
});

// node_modules/inherits/inherits_browser.js
var require_inherits_browser = __commonJS({
  "node_modules/inherits/inherits_browser.js"(exports, module) {
    if (typeof Object.create === "function") {
      module.exports = function inherits(ctor, superCtor) {
        if (superCtor) {
          ctor.super_ = superCtor;
          ctor.prototype = Object.create(superCtor.prototype, {
            constructor: {
              value: ctor,
              enumerable: false,
              writable: true,
              configurable: true
            }
          });
        }
      };
    } else {
      module.exports = function inherits(ctor, superCtor) {
        if (superCtor) {
          ctor.super_ = superCtor;
          var TempCtor = function() {
          };
          TempCtor.prototype = superCtor.prototype;
          ctor.prototype = new TempCtor();
          ctor.prototype.constructor = ctor;
        }
      };
    }
  }
});

// node_modules/inherits/inherits.js
var require_inherits = __commonJS({
  "node_modules/inherits/inherits.js"(exports, module) {
    try {
      util = __require("util");
      if (typeof util.inherits !== "function") throw "";
      module.exports = util.inherits;
    } catch (e) {
      module.exports = require_inherits_browser();
    }
    var util;
  }
});

// node_modules/safe-buffer/index.js
var require_safe_buffer = __commonJS({
  "node_modules/safe-buffer/index.js"(exports, module) {
    var buffer = __require("buffer");
    var Buffer4 = buffer.Buffer;
    function copyProps(src, dst) {
      for (var key in src) {
        dst[key] = src[key];
      }
    }
    if (Buffer4.from && Buffer4.alloc && Buffer4.allocUnsafe && Buffer4.allocUnsafeSlow) {
      module.exports = buffer;
    } else {
      copyProps(buffer, exports);
      exports.Buffer = SafeBuffer;
    }
    function SafeBuffer(arg, encodingOrOffset, length) {
      return Buffer4(arg, encodingOrOffset, length);
    }
    SafeBuffer.prototype = Object.create(Buffer4.prototype);
    copyProps(Buffer4, SafeBuffer);
    SafeBuffer.from = function(arg, encodingOrOffset, length) {
      if (typeof arg === "number") {
        throw new TypeError("Argument must not be a number");
      }
      return Buffer4(arg, encodingOrOffset, length);
    };
    SafeBuffer.alloc = function(size, fill, encoding) {
      if (typeof size !== "number") {
        throw new TypeError("Argument must be a number");
      }
      var buf = Buffer4(size);
      if (fill !== void 0) {
        if (typeof encoding === "string") {
          buf.fill(fill, encoding);
        } else {
          buf.fill(fill);
        }
      } else {
        buf.fill(0);
      }
      return buf;
    };
    SafeBuffer.allocUnsafe = function(size) {
      if (typeof size !== "number") {
        throw new TypeError("Argument must be a number");
      }
      return Buffer4(size);
    };
    SafeBuffer.allocUnsafeSlow = function(size) {
      if (typeof size !== "number") {
        throw new TypeError("Argument must be a number");
      }
      return buffer.SlowBuffer(size);
    };
  }
});

// node_modules/isarray/index.js
var require_isarray = __commonJS({
  "node_modules/isarray/index.js"(exports, module) {
    var toString = {}.toString;
    module.exports = Array.isArray || function(arr) {
      return toString.call(arr) == "[object Array]";
    };
  }
});

// node_modules/es-errors/type.js
var require_type = __commonJS({
  "node_modules/es-errors/type.js"(exports, module) {
    "use strict";
    module.exports = TypeError;
  }
});

// node_modules/es-object-atoms/index.js
var require_es_object_atoms = __commonJS({
  "node_modules/es-object-atoms/index.js"(exports, module) {
    "use strict";
    module.exports = Object;
  }
});

// node_modules/es-errors/index.js
var require_es_errors = __commonJS({
  "node_modules/es-errors/index.js"(exports, module) {
    "use strict";
    module.exports = Error;
  }
});

// node_modules/es-errors/eval.js
var require_eval = __commonJS({
  "node_modules/es-errors/eval.js"(exports, module) {
    "use strict";
    module.exports = EvalError;
  }
});

// node_modules/es-errors/range.js
var require_range = __commonJS({
  "node_modules/es-errors/range.js"(exports, module) {
    "use strict";
    module.exports = RangeError;
  }
});

// node_modules/es-errors/ref.js
var require_ref = __commonJS({
  "node_modules/es-errors/ref.js"(exports, module) {
    "use strict";
    module.exports = ReferenceError;
  }
});

// node_modules/es-errors/syntax.js
var require_syntax = __commonJS({
  "node_modules/es-errors/syntax.js"(exports, module) {
    "use strict";
    module.exports = SyntaxError;
  }
});

// node_modules/es-errors/uri.js
var require_uri = __commonJS({
  "node_modules/es-errors/uri.js"(exports, module) {
    "use strict";
    module.exports = URIError;
  }
});

// node_modules/math-intrinsics/abs.js
var require_abs = __commonJS({
  "node_modules/math-intrinsics/abs.js"(exports, module) {
    "use strict";
    module.exports = Math.abs;
  }
});

// node_modules/math-intrinsics/floor.js
var require_floor = __commonJS({
  "node_modules/math-intrinsics/floor.js"(exports, module) {
    "use strict";
    module.exports = Math.floor;
  }
});

// node_modules/math-intrinsics/max.js
var require_max = __commonJS({
  "node_modules/math-intrinsics/max.js"(exports, module) {
    "use strict";
    module.exports = Math.max;
  }
});

// node_modules/math-intrinsics/min.js
var require_min = __commonJS({
  "node_modules/math-intrinsics/min.js"(exports, module) {
    "use strict";
    module.exports = Math.min;
  }
});

// node_modules/math-intrinsics/pow.js
var require_pow = __commonJS({
  "node_modules/math-intrinsics/pow.js"(exports, module) {
    "use strict";
    module.exports = Math.pow;
  }
});

// node_modules/math-intrinsics/round.js
var require_round = __commonJS({
  "node_modules/math-intrinsics/round.js"(exports, module) {
    "use strict";
    module.exports = Math.round;
  }
});

// node_modules/math-intrinsics/isNaN.js
var require_isNaN = __commonJS({
  "node_modules/math-intrinsics/isNaN.js"(exports, module) {
    "use strict";
    module.exports = Number.isNaN || function isNaN2(a) {
      return a !== a;
    };
  }
});

// node_modules/math-intrinsics/sign.js
var require_sign = __commonJS({
  "node_modules/math-intrinsics/sign.js"(exports, module) {
    "use strict";
    var $isNaN = require_isNaN();
    module.exports = function sign(number) {
      if ($isNaN(number) || number === 0) {
        return number;
      }
      return number < 0 ? -1 : 1;
    };
  }
});

// node_modules/gopd/gOPD.js
var require_gOPD = __commonJS({
  "node_modules/gopd/gOPD.js"(exports, module) {
    "use strict";
    module.exports = Object.getOwnPropertyDescriptor;
  }
});

// node_modules/gopd/index.js
var require_gopd = __commonJS({
  "node_modules/gopd/index.js"(exports, module) {
    "use strict";
    var $gOPD = require_gOPD();
    if ($gOPD) {
      try {
        $gOPD([], "length");
      } catch (e) {
        $gOPD = null;
      }
    }
    module.exports = $gOPD;
  }
});

// node_modules/es-define-property/index.js
var require_es_define_property = __commonJS({
  "node_modules/es-define-property/index.js"(exports, module) {
    "use strict";
    var $defineProperty = Object.defineProperty || false;
    if ($defineProperty) {
      try {
        $defineProperty({}, "a", { value: 1 });
      } catch (e) {
        $defineProperty = false;
      }
    }
    module.exports = $defineProperty;
  }
});

// node_modules/has-symbols/shams.js
var require_shams = __commonJS({
  "node_modules/has-symbols/shams.js"(exports, module) {
    "use strict";
    module.exports = function hasSymbols() {
      if (typeof Symbol !== "function" || typeof Object.getOwnPropertySymbols !== "function") {
        return false;
      }
      if (typeof Symbol.iterator === "symbol") {
        return true;
      }
      var obj = {};
      var sym = Symbol("test");
      var symObj = Object(sym);
      if (typeof sym === "string") {
        return false;
      }
      if (Object.prototype.toString.call(sym) !== "[object Symbol]") {
        return false;
      }
      if (Object.prototype.toString.call(symObj) !== "[object Symbol]") {
        return false;
      }
      var symVal = 42;
      obj[sym] = symVal;
      for (var _ in obj) {
        return false;
      }
      if (typeof Object.keys === "function" && Object.keys(obj).length !== 0) {
        return false;
      }
      if (typeof Object.getOwnPropertyNames === "function" && Object.getOwnPropertyNames(obj).length !== 0) {
        return false;
      }
      var syms = Object.getOwnPropertySymbols(obj);
      if (syms.length !== 1 || syms[0] !== sym) {
        return false;
      }
      if (!Object.prototype.propertyIsEnumerable.call(obj, sym)) {
        return false;
      }
      if (typeof Object.getOwnPropertyDescriptor === "function") {
        var descriptor = (
          /** @type {PropertyDescriptor} */
          Object.getOwnPropertyDescriptor(obj, sym)
        );
        if (descriptor.value !== symVal || descriptor.enumerable !== true) {
          return false;
        }
      }
      return true;
    };
  }
});

// node_modules/has-symbols/index.js
var require_has_symbols = __commonJS({
  "node_modules/has-symbols/index.js"(exports, module) {
    "use strict";
    var origSymbol = typeof Symbol !== "undefined" && Symbol;
    var hasSymbolSham = require_shams();
    module.exports = function hasNativeSymbols() {
      if (typeof origSymbol !== "function") {
        return false;
      }
      if (typeof Symbol !== "function") {
        return false;
      }
      if (typeof origSymbol("foo") !== "symbol") {
        return false;
      }
      if (typeof Symbol("bar") !== "symbol") {
        return false;
      }
      return hasSymbolSham();
    };
  }
});

// node_modules/get-proto/Reflect.getPrototypeOf.js
var require_Reflect_getPrototypeOf = __commonJS({
  "node_modules/get-proto/Reflect.getPrototypeOf.js"(exports, module) {
    "use strict";
    module.exports = typeof Reflect !== "undefined" && Reflect.getPrototypeOf || null;
  }
});

// node_modules/get-proto/Object.getPrototypeOf.js
var require_Object_getPrototypeOf = __commonJS({
  "node_modules/get-proto/Object.getPrototypeOf.js"(exports, module) {
    "use strict";
    var $Object = require_es_object_atoms();
    module.exports = $Object.getPrototypeOf || null;
  }
});

// node_modules/function-bind/implementation.js
var require_implementation = __commonJS({
  "node_modules/function-bind/implementation.js"(exports, module) {
    "use strict";
    var ERROR_MESSAGE = "Function.prototype.bind called on incompatible ";
    var toStr = Object.prototype.toString;
    var max = Math.max;
    var funcType = "[object Function]";
    var concatty = function concatty2(a, b) {
      var arr = [];
      for (var i = 0; i < a.length; i += 1) {
        arr[i] = a[i];
      }
      for (var j = 0; j < b.length; j += 1) {
        arr[j + a.length] = b[j];
      }
      return arr;
    };
    var slicy = function slicy2(arrLike, offset) {
      var arr = [];
      for (var i = offset || 0, j = 0; i < arrLike.length; i += 1, j += 1) {
        arr[j] = arrLike[i];
      }
      return arr;
    };
    var joiny = function(arr, joiner) {
      var str = "";
      for (var i = 0; i < arr.length; i += 1) {
        str += arr[i];
        if (i + 1 < arr.length) {
          str += joiner;
        }
      }
      return str;
    };
    module.exports = function bind(that) {
      var target = this;
      if (typeof target !== "function" || toStr.apply(target) !== funcType) {
        throw new TypeError(ERROR_MESSAGE + target);
      }
      var args = slicy(arguments, 1);
      var bound;
      var binder = function() {
        if (this instanceof bound) {
          var result = target.apply(
            this,
            concatty(args, arguments)
          );
          if (Object(result) === result) {
            return result;
          }
          return this;
        }
        return target.apply(
          that,
          concatty(args, arguments)
        );
      };
      var boundLength = max(0, target.length - args.length);
      var boundArgs = [];
      for (var i = 0; i < boundLength; i++) {
        boundArgs[i] = "$" + i;
      }
      bound = Function("binder", "return function (" + joiny(boundArgs, ",") + "){ return binder.apply(this,arguments); }")(binder);
      if (target.prototype) {
        var Empty = function Empty2() {
        };
        Empty.prototype = target.prototype;
        bound.prototype = new Empty();
        Empty.prototype = null;
      }
      return bound;
    };
  }
});

// node_modules/function-bind/index.js
var require_function_bind = __commonJS({
  "node_modules/function-bind/index.js"(exports, module) {
    "use strict";
    var implementation = require_implementation();
    module.exports = Function.prototype.bind || implementation;
  }
});

// node_modules/call-bind-apply-helpers/functionCall.js
var require_functionCall = __commonJS({
  "node_modules/call-bind-apply-helpers/functionCall.js"(exports, module) {
    "use strict";
    module.exports = Function.prototype.call;
  }
});

// node_modules/call-bind-apply-helpers/functionApply.js
var require_functionApply = __commonJS({
  "node_modules/call-bind-apply-helpers/functionApply.js"(exports, module) {
    "use strict";
    module.exports = Function.prototype.apply;
  }
});

// node_modules/call-bind-apply-helpers/reflectApply.js
var require_reflectApply = __commonJS({
  "node_modules/call-bind-apply-helpers/reflectApply.js"(exports, module) {
    "use strict";
    module.exports = typeof Reflect !== "undefined" && Reflect && Reflect.apply;
  }
});

// node_modules/call-bind-apply-helpers/actualApply.js
var require_actualApply = __commonJS({
  "node_modules/call-bind-apply-helpers/actualApply.js"(exports, module) {
    "use strict";
    var bind = require_function_bind();
    var $apply = require_functionApply();
    var $call = require_functionCall();
    var $reflectApply = require_reflectApply();
    module.exports = $reflectApply || bind.call($call, $apply);
  }
});

// node_modules/call-bind-apply-helpers/index.js
var require_call_bind_apply_helpers = __commonJS({
  "node_modules/call-bind-apply-helpers/index.js"(exports, module) {
    "use strict";
    var bind = require_function_bind();
    var $TypeError = require_type();
    var $call = require_functionCall();
    var $actualApply = require_actualApply();
    module.exports = function callBindBasic(args) {
      if (args.length < 1 || typeof args[0] !== "function") {
        throw new $TypeError("a function is required");
      }
      return $actualApply(bind, $call, args);
    };
  }
});

// node_modules/dunder-proto/get.js
var require_get = __commonJS({
  "node_modules/dunder-proto/get.js"(exports, module) {
    "use strict";
    var callBind = require_call_bind_apply_helpers();
    var gOPD = require_gopd();
    var hasProtoAccessor;
    try {
      hasProtoAccessor = /** @type {{ __proto__?: typeof Array.prototype }} */
      [].__proto__ === Array.prototype;
    } catch (e) {
      if (!e || typeof e !== "object" || !("code" in e) || e.code !== "ERR_PROTO_ACCESS") {
        throw e;
      }
    }
    var desc = !!hasProtoAccessor && gOPD && gOPD(
      Object.prototype,
      /** @type {keyof typeof Object.prototype} */
      "__proto__"
    );
    var $Object = Object;
    var $getPrototypeOf = $Object.getPrototypeOf;
    module.exports = desc && typeof desc.get === "function" ? callBind([desc.get]) : typeof $getPrototypeOf === "function" ? (
      /** @type {import('./get')} */
      function getDunder(value) {
        return $getPrototypeOf(value == null ? value : $Object(value));
      }
    ) : false;
  }
});

// node_modules/get-proto/index.js
var require_get_proto = __commonJS({
  "node_modules/get-proto/index.js"(exports, module) {
    "use strict";
    var reflectGetProto = require_Reflect_getPrototypeOf();
    var originalGetProto = require_Object_getPrototypeOf();
    var getDunderProto = require_get();
    module.exports = reflectGetProto ? function getProto(O) {
      return reflectGetProto(O);
    } : originalGetProto ? function getProto(O) {
      if (!O || typeof O !== "object" && typeof O !== "function") {
        throw new TypeError("getProto: not an object");
      }
      return originalGetProto(O);
    } : getDunderProto ? function getProto(O) {
      return getDunderProto(O);
    } : null;
  }
});

// node_modules/hasown/index.js
var require_hasown = __commonJS({
  "node_modules/hasown/index.js"(exports, module) {
    "use strict";
    var call = Function.prototype.call;
    var $hasOwn = Object.prototype.hasOwnProperty;
    var bind = require_function_bind();
    module.exports = bind.call(call, $hasOwn);
  }
});

// node_modules/get-intrinsic/index.js
var require_get_intrinsic = __commonJS({
  "node_modules/get-intrinsic/index.js"(exports, module) {
    "use strict";
    var undefined2;
    var $Object = require_es_object_atoms();
    var $Error = require_es_errors();
    var $EvalError = require_eval();
    var $RangeError = require_range();
    var $ReferenceError = require_ref();
    var $SyntaxError = require_syntax();
    var $TypeError = require_type();
    var $URIError = require_uri();
    var abs = require_abs();
    var floor = require_floor();
    var max = require_max();
    var min = require_min();
    var pow = require_pow();
    var round = require_round();
    var sign = require_sign();
    var $Function = Function;
    var getEvalledConstructor = function(expressionSyntax) {
      try {
        return $Function('"use strict"; return (' + expressionSyntax + ").constructor;")();
      } catch (e) {
      }
    };
    var $gOPD = require_gopd();
    var $defineProperty = require_es_define_property();
    var throwTypeError = function() {
      throw new $TypeError();
    };
    var ThrowTypeError = $gOPD ? function() {
      try {
        arguments.callee;
        return throwTypeError;
      } catch (calleeThrows) {
        try {
          return $gOPD(arguments, "callee").get;
        } catch (gOPDthrows) {
          return throwTypeError;
        }
      }
    }() : throwTypeError;
    var hasSymbols = require_has_symbols()();
    var getProto = require_get_proto();
    var $ObjectGPO = require_Object_getPrototypeOf();
    var $ReflectGPO = require_Reflect_getPrototypeOf();
    var $apply = require_functionApply();
    var $call = require_functionCall();
    var needsEval = {};
    var TypedArray = typeof Uint8Array === "undefined" || !getProto ? undefined2 : getProto(Uint8Array);
    var INTRINSICS = {
      __proto__: null,
      "%AggregateError%": typeof AggregateError === "undefined" ? undefined2 : AggregateError,
      "%Array%": Array,
      "%ArrayBuffer%": typeof ArrayBuffer === "undefined" ? undefined2 : ArrayBuffer,
      "%ArrayIteratorPrototype%": hasSymbols && getProto ? getProto([][Symbol.iterator]()) : undefined2,
      "%AsyncFromSyncIteratorPrototype%": undefined2,
      "%AsyncFunction%": needsEval,
      "%AsyncGenerator%": needsEval,
      "%AsyncGeneratorFunction%": needsEval,
      "%AsyncIteratorPrototype%": needsEval,
      "%Atomics%": typeof Atomics === "undefined" ? undefined2 : Atomics,
      "%BigInt%": typeof BigInt === "undefined" ? undefined2 : BigInt,
      "%BigInt64Array%": typeof BigInt64Array === "undefined" ? undefined2 : BigInt64Array,
      "%BigUint64Array%": typeof BigUint64Array === "undefined" ? undefined2 : BigUint64Array,
      "%Boolean%": Boolean,
      "%DataView%": typeof DataView === "undefined" ? undefined2 : DataView,
      "%Date%": Date,
      "%decodeURI%": decodeURI,
      "%decodeURIComponent%": decodeURIComponent,
      "%encodeURI%": encodeURI,
      "%encodeURIComponent%": encodeURIComponent,
      "%Error%": $Error,
      "%eval%": eval,
      // eslint-disable-line no-eval
      "%EvalError%": $EvalError,
      "%Float16Array%": typeof Float16Array === "undefined" ? undefined2 : Float16Array,
      "%Float32Array%": typeof Float32Array === "undefined" ? undefined2 : Float32Array,
      "%Float64Array%": typeof Float64Array === "undefined" ? undefined2 : Float64Array,
      "%FinalizationRegistry%": typeof FinalizationRegistry === "undefined" ? undefined2 : FinalizationRegistry,
      "%Function%": $Function,
      "%GeneratorFunction%": needsEval,
      "%Int8Array%": typeof Int8Array === "undefined" ? undefined2 : Int8Array,
      "%Int16Array%": typeof Int16Array === "undefined" ? undefined2 : Int16Array,
      "%Int32Array%": typeof Int32Array === "undefined" ? undefined2 : Int32Array,
      "%isFinite%": isFinite,
      "%isNaN%": isNaN,
      "%IteratorPrototype%": hasSymbols && getProto ? getProto(getProto([][Symbol.iterator]())) : undefined2,
      "%JSON%": typeof JSON === "object" ? JSON : undefined2,
      "%Map%": typeof Map === "undefined" ? undefined2 : Map,
      "%MapIteratorPrototype%": typeof Map === "undefined" || !hasSymbols || !getProto ? undefined2 : getProto((/* @__PURE__ */ new Map())[Symbol.iterator]()),
      "%Math%": Math,
      "%Number%": Number,
      "%Object%": $Object,
      "%Object.getOwnPropertyDescriptor%": $gOPD,
      "%parseFloat%": parseFloat,
      "%parseInt%": parseInt,
      "%Promise%": typeof Promise === "undefined" ? undefined2 : Promise,
      "%Proxy%": typeof Proxy === "undefined" ? undefined2 : Proxy,
      "%RangeError%": $RangeError,
      "%ReferenceError%": $ReferenceError,
      "%Reflect%": typeof Reflect === "undefined" ? undefined2 : Reflect,
      "%RegExp%": RegExp,
      "%Set%": typeof Set === "undefined" ? undefined2 : Set,
      "%SetIteratorPrototype%": typeof Set === "undefined" || !hasSymbols || !getProto ? undefined2 : getProto((/* @__PURE__ */ new Set())[Symbol.iterator]()),
      "%SharedArrayBuffer%": typeof SharedArrayBuffer === "undefined" ? undefined2 : SharedArrayBuffer,
      "%String%": String,
      "%StringIteratorPrototype%": hasSymbols && getProto ? getProto(""[Symbol.iterator]()) : undefined2,
      "%Symbol%": hasSymbols ? Symbol : undefined2,
      "%SyntaxError%": $SyntaxError,
      "%ThrowTypeError%": ThrowTypeError,
      "%TypedArray%": TypedArray,
      "%TypeError%": $TypeError,
      "%Uint8Array%": typeof Uint8Array === "undefined" ? undefined2 : Uint8Array,
      "%Uint8ClampedArray%": typeof Uint8ClampedArray === "undefined" ? undefined2 : Uint8ClampedArray,
      "%Uint16Array%": typeof Uint16Array === "undefined" ? undefined2 : Uint16Array,
      "%Uint32Array%": typeof Uint32Array === "undefined" ? undefined2 : Uint32Array,
      "%URIError%": $URIError,
      "%WeakMap%": typeof WeakMap === "undefined" ? undefined2 : WeakMap,
      "%WeakRef%": typeof WeakRef === "undefined" ? undefined2 : WeakRef,
      "%WeakSet%": typeof WeakSet === "undefined" ? undefined2 : WeakSet,
      "%Function.prototype.call%": $call,
      "%Function.prototype.apply%": $apply,
      "%Object.defineProperty%": $defineProperty,
      "%Object.getPrototypeOf%": $ObjectGPO,
      "%Math.abs%": abs,
      "%Math.floor%": floor,
      "%Math.max%": max,
      "%Math.min%": min,
      "%Math.pow%": pow,
      "%Math.round%": round,
      "%Math.sign%": sign,
      "%Reflect.getPrototypeOf%": $ReflectGPO
    };
    if (getProto) {
      try {
        null.error;
      } catch (e) {
        errorProto = getProto(getProto(e));
        INTRINSICS["%Error.prototype%"] = errorProto;
      }
    }
    var errorProto;
    var doEval = function doEval2(name) {
      var value;
      if (name === "%AsyncFunction%") {
        value = getEvalledConstructor("async function () {}");
      } else if (name === "%GeneratorFunction%") {
        value = getEvalledConstructor("function* () {}");
      } else if (name === "%AsyncGeneratorFunction%") {
        value = getEvalledConstructor("async function* () {}");
      } else if (name === "%AsyncGenerator%") {
        var fn = doEval2("%AsyncGeneratorFunction%");
        if (fn) {
          value = fn.prototype;
        }
      } else if (name === "%AsyncIteratorPrototype%") {
        var gen = doEval2("%AsyncGenerator%");
        if (gen && getProto) {
          value = getProto(gen.prototype);
        }
      }
      INTRINSICS[name] = value;
      return value;
    };
    var LEGACY_ALIASES = {
      __proto__: null,
      "%ArrayBufferPrototype%": ["ArrayBuffer", "prototype"],
      "%ArrayPrototype%": ["Array", "prototype"],
      "%ArrayProto_entries%": ["Array", "prototype", "entries"],
      "%ArrayProto_forEach%": ["Array", "prototype", "forEach"],
      "%ArrayProto_keys%": ["Array", "prototype", "keys"],
      "%ArrayProto_values%": ["Array", "prototype", "values"],
      "%AsyncFunctionPrototype%": ["AsyncFunction", "prototype"],
      "%AsyncGenerator%": ["AsyncGeneratorFunction", "prototype"],
      "%AsyncGeneratorPrototype%": ["AsyncGeneratorFunction", "prototype", "prototype"],
      "%BooleanPrototype%": ["Boolean", "prototype"],
      "%DataViewPrototype%": ["DataView", "prototype"],
      "%DatePrototype%": ["Date", "prototype"],
      "%ErrorPrototype%": ["Error", "prototype"],
      "%EvalErrorPrototype%": ["EvalError", "prototype"],
      "%Float32ArrayPrototype%": ["Float32Array", "prototype"],
      "%Float64ArrayPrototype%": ["Float64Array", "prototype"],
      "%FunctionPrototype%": ["Function", "prototype"],
      "%Generator%": ["GeneratorFunction", "prototype"],
      "%GeneratorPrototype%": ["GeneratorFunction", "prototype", "prototype"],
      "%Int8ArrayPrototype%": ["Int8Array", "prototype"],
      "%Int16ArrayPrototype%": ["Int16Array", "prototype"],
      "%Int32ArrayPrototype%": ["Int32Array", "prototype"],
      "%JSONParse%": ["JSON", "parse"],
      "%JSONStringify%": ["JSON", "stringify"],
      "%MapPrototype%": ["Map", "prototype"],
      "%NumberPrototype%": ["Number", "prototype"],
      "%ObjectPrototype%": ["Object", "prototype"],
      "%ObjProto_toString%": ["Object", "prototype", "toString"],
      "%ObjProto_valueOf%": ["Object", "prototype", "valueOf"],
      "%PromisePrototype%": ["Promise", "prototype"],
      "%PromiseProto_then%": ["Promise", "prototype", "then"],
      "%Promise_all%": ["Promise", "all"],
      "%Promise_reject%": ["Promise", "reject"],
      "%Promise_resolve%": ["Promise", "resolve"],
      "%RangeErrorPrototype%": ["RangeError", "prototype"],
      "%ReferenceErrorPrototype%": ["ReferenceError", "prototype"],
      "%RegExpPrototype%": ["RegExp", "prototype"],
      "%SetPrototype%": ["Set", "prototype"],
      "%SharedArrayBufferPrototype%": ["SharedArrayBuffer", "prototype"],
      "%StringPrototype%": ["String", "prototype"],
      "%SymbolPrototype%": ["Symbol", "prototype"],
      "%SyntaxErrorPrototype%": ["SyntaxError", "prototype"],
      "%TypedArrayPrototype%": ["TypedArray", "prototype"],
      "%TypeErrorPrototype%": ["TypeError", "prototype"],
      "%Uint8ArrayPrototype%": ["Uint8Array", "prototype"],
      "%Uint8ClampedArrayPrototype%": ["Uint8ClampedArray", "prototype"],
      "%Uint16ArrayPrototype%": ["Uint16Array", "prototype"],
      "%Uint32ArrayPrototype%": ["Uint32Array", "prototype"],
      "%URIErrorPrototype%": ["URIError", "prototype"],
      "%WeakMapPrototype%": ["WeakMap", "prototype"],
      "%WeakSetPrototype%": ["WeakSet", "prototype"]
    };
    var bind = require_function_bind();
    var hasOwn = require_hasown();
    var $concat = bind.call($call, Array.prototype.concat);
    var $spliceApply = bind.call($apply, Array.prototype.splice);
    var $replace = bind.call($call, String.prototype.replace);
    var $strSlice = bind.call($call, String.prototype.slice);
    var $exec = bind.call($call, RegExp.prototype.exec);
    var rePropName = /[^%.[\]]+|\[(?:(-?\d+(?:\.\d+)?)|(["'])((?:(?!\2)[^\\]|\\.)*?)\2)\]|(?=(?:\.|\[\])(?:\.|\[\]|%$))/g;
    var reEscapeChar = /\\(\\)?/g;
    var stringToPath = function stringToPath2(string) {
      var first = $strSlice(string, 0, 1);
      var last = $strSlice(string, -1);
      if (first === "%" && last !== "%") {
        throw new $SyntaxError("invalid intrinsic syntax, expected closing `%`");
      } else if (last === "%" && first !== "%") {
        throw new $SyntaxError("invalid intrinsic syntax, expected opening `%`");
      }
      var result = [];
      $replace(string, rePropName, function(match, number, quote, subString) {
        result[result.length] = quote ? $replace(subString, reEscapeChar, "$1") : number || match;
      });
      return result;
    };
    var getBaseIntrinsic = function getBaseIntrinsic2(name, allowMissing) {
      var intrinsicName = name;
      var alias;
      if (hasOwn(LEGACY_ALIASES, intrinsicName)) {
        alias = LEGACY_ALIASES[intrinsicName];
        intrinsicName = "%" + alias[0] + "%";
      }
      if (hasOwn(INTRINSICS, intrinsicName)) {
        var value = INTRINSICS[intrinsicName];
        if (value === needsEval) {
          value = doEval(intrinsicName);
        }
        if (typeof value === "undefined" && !allowMissing) {
          throw new $TypeError("intrinsic " + name + " exists, but is not available. Please file an issue!");
        }
        return {
          alias,
          name: intrinsicName,
          value
        };
      }
      throw new $SyntaxError("intrinsic " + name + " does not exist!");
    };
    module.exports = function GetIntrinsic(name, allowMissing) {
      if (typeof name !== "string" || name.length === 0) {
        throw new $TypeError("intrinsic name must be a non-empty string");
      }
      if (arguments.length > 1 && typeof allowMissing !== "boolean") {
        throw new $TypeError('"allowMissing" argument must be a boolean');
      }
      if ($exec(/^%?[^%]*%?$/, name) === null) {
        throw new $SyntaxError("`%` may not be present anywhere but at the beginning and end of the intrinsic name");
      }
      var parts = stringToPath(name);
      var intrinsicBaseName = parts.length > 0 ? parts[0] : "";
      var intrinsic = getBaseIntrinsic("%" + intrinsicBaseName + "%", allowMissing);
      var intrinsicRealName = intrinsic.name;
      var value = intrinsic.value;
      var skipFurtherCaching = false;
      var alias = intrinsic.alias;
      if (alias) {
        intrinsicBaseName = alias[0];
        $spliceApply(parts, $concat([0, 1], alias));
      }
      for (var i = 1, isOwn = true; i < parts.length; i += 1) {
        var part = parts[i];
        var first = $strSlice(part, 0, 1);
        var last = $strSlice(part, -1);
        if ((first === '"' || first === "'" || first === "`" || (last === '"' || last === "'" || last === "`")) && first !== last) {
          throw new $SyntaxError("property names with quotes must have matching quotes");
        }
        if (part === "constructor" || !isOwn) {
          skipFurtherCaching = true;
        }
        intrinsicBaseName += "." + part;
        intrinsicRealName = "%" + intrinsicBaseName + "%";
        if (hasOwn(INTRINSICS, intrinsicRealName)) {
          value = INTRINSICS[intrinsicRealName];
        } else if (value != null) {
          if (!(part in value)) {
            if (!allowMissing) {
              throw new $TypeError("base intrinsic for " + name + " exists, but the property is not available.");
            }
            return void 0;
          }
          if ($gOPD && i + 1 >= parts.length) {
            var desc = $gOPD(value, part);
            isOwn = !!desc;
            if (isOwn && "get" in desc && !("originalValue" in desc.get)) {
              value = desc.get;
            } else {
              value = value[part];
            }
          } else {
            isOwn = hasOwn(value, part);
            value = value[part];
          }
          if (isOwn && !skipFurtherCaching) {
            INTRINSICS[intrinsicRealName] = value;
          }
        }
      }
      return value;
    };
  }
});

// node_modules/call-bound/index.js
var require_call_bound = __commonJS({
  "node_modules/call-bound/index.js"(exports, module) {
    "use strict";
    var GetIntrinsic = require_get_intrinsic();
    var callBindBasic = require_call_bind_apply_helpers();
    var $indexOf = callBindBasic([GetIntrinsic("%String.prototype.indexOf%")]);
    module.exports = function callBoundIntrinsic(name, allowMissing) {
      var intrinsic = (
        /** @type {(this: unknown, ...args: unknown[]) => unknown} */
        GetIntrinsic(name, !!allowMissing)
      );
      if (typeof intrinsic === "function" && $indexOf(name, ".prototype.") > -1) {
        return callBindBasic(
          /** @type {const} */
          [intrinsic]
        );
      }
      return intrinsic;
    };
  }
});

// node_modules/is-callable/index.js
var require_is_callable = __commonJS({
  "node_modules/is-callable/index.js"(exports, module) {
    "use strict";
    var fnToStr = Function.prototype.toString;
    var reflectApply = typeof Reflect === "object" && Reflect !== null && Reflect.apply;
    var badArrayLike;
    var isCallableMarker;
    if (typeof reflectApply === "function" && typeof Object.defineProperty === "function") {
      try {
        badArrayLike = Object.defineProperty({}, "length", {
          get: function() {
            throw isCallableMarker;
          }
        });
        isCallableMarker = {};
        reflectApply(function() {
          throw 42;
        }, null, badArrayLike);
      } catch (_) {
        if (_ !== isCallableMarker) {
          reflectApply = null;
        }
      }
    } else {
      reflectApply = null;
    }
    var constructorRegex = /^\s*class\b/;
    var isES6ClassFn = function isES6ClassFunction(value) {
      try {
        var fnStr = fnToStr.call(value);
        return constructorRegex.test(fnStr);
      } catch (e) {
        return false;
      }
    };
    var tryFunctionObject = function tryFunctionToStr(value) {
      try {
        if (isES6ClassFn(value)) {
          return false;
        }
        fnToStr.call(value);
        return true;
      } catch (e) {
        return false;
      }
    };
    var toStr = Object.prototype.toString;
    var objectClass = "[object Object]";
    var fnClass = "[object Function]";
    var genClass = "[object GeneratorFunction]";
    var ddaClass = "[object HTMLAllCollection]";
    var ddaClass2 = "[object HTML document.all class]";
    var ddaClass3 = "[object HTMLCollection]";
    var hasToStringTag = typeof Symbol === "function" && !!Symbol.toStringTag;
    var isIE68 = !(0 in [,]);
    var isDDA = function isDocumentDotAll() {
      return false;
    };
    if (typeof document === "object") {
      all = document.all;
      if (toStr.call(all) === toStr.call(document.all)) {
        isDDA = function isDocumentDotAll(value) {
          if ((isIE68 || !value) && (typeof value === "undefined" || typeof value === "object")) {
            try {
              var str = toStr.call(value);
              return (str === ddaClass || str === ddaClass2 || str === ddaClass3 || str === objectClass) && value("") == null;
            } catch (e) {
            }
          }
          return false;
        };
      }
    }
    var all;
    module.exports = reflectApply ? function isCallable(value) {
      if (isDDA(value)) {
        return true;
      }
      if (!value) {
        return false;
      }
      if (typeof value !== "function" && typeof value !== "object") {
        return false;
      }
      try {
        reflectApply(value, null, badArrayLike);
      } catch (e) {
        if (e !== isCallableMarker) {
          return false;
        }
      }
      return !isES6ClassFn(value) && tryFunctionObject(value);
    } : function isCallable(value) {
      if (isDDA(value)) {
        return true;
      }
      if (!value) {
        return false;
      }
      if (typeof value !== "function" && typeof value !== "object") {
        return false;
      }
      if (hasToStringTag) {
        return tryFunctionObject(value);
      }
      if (isES6ClassFn(value)) {
        return false;
      }
      var strClass = toStr.call(value);
      if (strClass !== fnClass && strClass !== genClass && !/^\[object HTML/.test(strClass)) {
        return false;
      }
      return tryFunctionObject(value);
    };
  }
});

// node_modules/for-each/index.js
var require_for_each = __commonJS({
  "node_modules/for-each/index.js"(exports, module) {
    "use strict";
    var isCallable = require_is_callable();
    var toStr = Object.prototype.toString;
    var hasOwnProperty = Object.prototype.hasOwnProperty;
    var forEachArray = function forEachArray2(array, iterator, receiver) {
      for (var i = 0, len = array.length; i < len; i++) {
        if (hasOwnProperty.call(array, i)) {
          if (receiver == null) {
            iterator(array[i], i, array);
          } else {
            iterator.call(receiver, array[i], i, array);
          }
        }
      }
    };
    var forEachString = function forEachString2(string, iterator, receiver) {
      for (var i = 0, len = string.length; i < len; i++) {
        if (receiver == null) {
          iterator(string.charAt(i), i, string);
        } else {
          iterator.call(receiver, string.charAt(i), i, string);
        }
      }
    };
    var forEachObject = function forEachObject2(object, iterator, receiver) {
      for (var k in object) {
        if (hasOwnProperty.call(object, k)) {
          if (receiver == null) {
            iterator(object[k], k, object);
          } else {
            iterator.call(receiver, object[k], k, object);
          }
        }
      }
    };
    function isArray(x) {
      return toStr.call(x) === "[object Array]";
    }
    module.exports = function forEach(list, iterator, thisArg) {
      if (!isCallable(iterator)) {
        throw new TypeError("iterator must be a function");
      }
      var receiver;
      if (arguments.length >= 3) {
        receiver = thisArg;
      }
      if (isArray(list)) {
        forEachArray(list, iterator, receiver);
      } else if (typeof list === "string") {
        forEachString(list, iterator, receiver);
      } else {
        forEachObject(list, iterator, receiver);
      }
    };
  }
});

// node_modules/possible-typed-array-names/index.js
var require_possible_typed_array_names = __commonJS({
  "node_modules/possible-typed-array-names/index.js"(exports, module) {
    "use strict";
    module.exports = [
      "Float16Array",
      "Float32Array",
      "Float64Array",
      "Int8Array",
      "Int16Array",
      "Int32Array",
      "Uint8Array",
      "Uint8ClampedArray",
      "Uint16Array",
      "Uint32Array",
      "BigInt64Array",
      "BigUint64Array"
    ];
  }
});

// node_modules/available-typed-arrays/index.js
var require_available_typed_arrays = __commonJS({
  "node_modules/available-typed-arrays/index.js"(exports, module) {
    "use strict";
    var possibleNames = require_possible_typed_array_names();
    var g = typeof globalThis === "undefined" ? global : globalThis;
    module.exports = function availableTypedArrays() {
      var out = [];
      for (var i = 0; i < possibleNames.length; i++) {
        if (typeof g[possibleNames[i]] === "function") {
          out[out.length] = possibleNames[i];
        }
      }
      return out;
    };
  }
});

// node_modules/define-data-property/index.js
var require_define_data_property = __commonJS({
  "node_modules/define-data-property/index.js"(exports, module) {
    "use strict";
    var $defineProperty = require_es_define_property();
    var $SyntaxError = require_syntax();
    var $TypeError = require_type();
    var gopd = require_gopd();
    module.exports = function defineDataProperty(obj, property, value) {
      if (!obj || typeof obj !== "object" && typeof obj !== "function") {
        throw new $TypeError("`obj` must be an object or a function`");
      }
      if (typeof property !== "string" && typeof property !== "symbol") {
        throw new $TypeError("`property` must be a string or a symbol`");
      }
      if (arguments.length > 3 && typeof arguments[3] !== "boolean" && arguments[3] !== null) {
        throw new $TypeError("`nonEnumerable`, if provided, must be a boolean or null");
      }
      if (arguments.length > 4 && typeof arguments[4] !== "boolean" && arguments[4] !== null) {
        throw new $TypeError("`nonWritable`, if provided, must be a boolean or null");
      }
      if (arguments.length > 5 && typeof arguments[5] !== "boolean" && arguments[5] !== null) {
        throw new $TypeError("`nonConfigurable`, if provided, must be a boolean or null");
      }
      if (arguments.length > 6 && typeof arguments[6] !== "boolean") {
        throw new $TypeError("`loose`, if provided, must be a boolean");
      }
      var nonEnumerable = arguments.length > 3 ? arguments[3] : null;
      var nonWritable = arguments.length > 4 ? arguments[4] : null;
      var nonConfigurable = arguments.length > 5 ? arguments[5] : null;
      var loose = arguments.length > 6 ? arguments[6] : false;
      var desc = !!gopd && gopd(obj, property);
      if ($defineProperty) {
        $defineProperty(obj, property, {
          configurable: nonConfigurable === null && desc ? desc.configurable : !nonConfigurable,
          enumerable: nonEnumerable === null && desc ? desc.enumerable : !nonEnumerable,
          value,
          writable: nonWritable === null && desc ? desc.writable : !nonWritable
        });
      } else if (loose || !nonEnumerable && !nonWritable && !nonConfigurable) {
        obj[property] = value;
      } else {
        throw new $SyntaxError("This environment does not support defining a property as non-configurable, non-writable, or non-enumerable.");
      }
    };
  }
});

// node_modules/has-property-descriptors/index.js
var require_has_property_descriptors = __commonJS({
  "node_modules/has-property-descriptors/index.js"(exports, module) {
    "use strict";
    var $defineProperty = require_es_define_property();
    var hasPropertyDescriptors = function hasPropertyDescriptors2() {
      return !!$defineProperty;
    };
    hasPropertyDescriptors.hasArrayLengthDefineBug = function hasArrayLengthDefineBug() {
      if (!$defineProperty) {
        return null;
      }
      try {
        return $defineProperty([], "length", { value: 1 }).length !== 1;
      } catch (e) {
        return true;
      }
    };
    module.exports = hasPropertyDescriptors;
  }
});

// node_modules/set-function-length/index.js
var require_set_function_length = __commonJS({
  "node_modules/set-function-length/index.js"(exports, module) {
    "use strict";
    var GetIntrinsic = require_get_intrinsic();
    var define2 = require_define_data_property();
    var hasDescriptors = require_has_property_descriptors()();
    var gOPD = require_gopd();
    var $TypeError = require_type();
    var $floor = GetIntrinsic("%Math.floor%");
    module.exports = function setFunctionLength(fn, length) {
      if (typeof fn !== "function") {
        throw new $TypeError("`fn` is not a function");
      }
      if (typeof length !== "number" || length < 0 || length > 4294967295 || $floor(length) !== length) {
        throw new $TypeError("`length` must be a positive 32-bit integer");
      }
      var loose = arguments.length > 2 && !!arguments[2];
      var functionLengthIsConfigurable = true;
      var functionLengthIsWritable = true;
      if ("length" in fn && gOPD) {
        var desc = gOPD(fn, "length");
        if (desc && !desc.configurable) {
          functionLengthIsConfigurable = false;
        }
        if (desc && !desc.writable) {
          functionLengthIsWritable = false;
        }
      }
      if (functionLengthIsConfigurable || functionLengthIsWritable || !loose) {
        if (hasDescriptors) {
          define2(
            /** @type {Parameters<define>[0]} */
            fn,
            "length",
            length,
            true,
            true
          );
        } else {
          define2(
            /** @type {Parameters<define>[0]} */
            fn,
            "length",
            length
          );
        }
      }
      return fn;
    };
  }
});

// node_modules/call-bind-apply-helpers/applyBind.js
var require_applyBind = __commonJS({
  "node_modules/call-bind-apply-helpers/applyBind.js"(exports, module) {
    "use strict";
    var bind = require_function_bind();
    var $apply = require_functionApply();
    var actualApply = require_actualApply();
    module.exports = function applyBind() {
      return actualApply(bind, $apply, arguments);
    };
  }
});

// node_modules/call-bind/index.js
var require_call_bind = __commonJS({
  "node_modules/call-bind/index.js"(exports, module) {
    "use strict";
    var setFunctionLength = require_set_function_length();
    var $defineProperty = require_es_define_property();
    var callBindBasic = require_call_bind_apply_helpers();
    var applyBind = require_applyBind();
    module.exports = function callBind(originalFunction) {
      var func = callBindBasic(arguments);
      var adjustedLength = 1 + originalFunction.length - (arguments.length - 1);
      return setFunctionLength(
        func,
        adjustedLength > 0 ? adjustedLength : 0,
        true
      );
    };
    if ($defineProperty) {
      $defineProperty(module.exports, "apply", { value: applyBind });
    } else {
      module.exports.apply = applyBind;
    }
  }
});

// node_modules/has-tostringtag/shams.js
var require_shams2 = __commonJS({
  "node_modules/has-tostringtag/shams.js"(exports, module) {
    "use strict";
    var hasSymbols = require_shams();
    module.exports = function hasToStringTagShams() {
      return hasSymbols() && !!Symbol.toStringTag;
    };
  }
});

// node_modules/which-typed-array/index.js
var require_which_typed_array = __commonJS({
  "node_modules/which-typed-array/index.js"(exports, module) {
    "use strict";
    var forEach = require_for_each();
    var availableTypedArrays = require_available_typed_arrays();
    var callBind = require_call_bind();
    var callBound = require_call_bound();
    var gOPD = require_gopd();
    var getProto = require_get_proto();
    var $toString = callBound("Object.prototype.toString");
    var hasToStringTag = require_shams2()();
    var g = typeof globalThis === "undefined" ? global : globalThis;
    var typedArrays = availableTypedArrays();
    var $slice = callBound("String.prototype.slice");
    var $indexOf = callBound("Array.prototype.indexOf", true) || function indexOf(array, value) {
      for (var i = 0; i < array.length; i += 1) {
        if (array[i] === value) {
          return i;
        }
      }
      return -1;
    };
    var cache = { __proto__: null };
    if (hasToStringTag && gOPD && getProto) {
      forEach(typedArrays, function(typedArray) {
        var arr = new g[typedArray]();
        if (Symbol.toStringTag in arr && getProto) {
          var proto = getProto(arr);
          var descriptor = gOPD(proto, Symbol.toStringTag);
          if (!descriptor && proto) {
            var superProto = getProto(proto);
            descriptor = gOPD(superProto, Symbol.toStringTag);
          }
          if (descriptor && descriptor.get) {
            var bound = callBind(descriptor.get);
            cache[
              /** @type {`$${import('.').TypedArrayName}`} */
              "$" + typedArray
            ] = bound;
          }
        }
      });
    } else {
      forEach(typedArrays, function(typedArray) {
        var arr = new g[typedArray]();
        var fn = arr.slice || arr.set;
        if (fn) {
          var bound = (
            /** @type {import('./types').BoundSlice | import('./types').BoundSet} */
            // @ts-expect-error TODO FIXME
            callBind(fn)
          );
          cache[
            /** @type {`$${import('.').TypedArrayName}`} */
            "$" + typedArray
          ] = bound;
        }
      });
    }
    var tryTypedArrays = function tryAllTypedArrays(value) {
      var found = false;
      forEach(
        /** @type {Record<`\$${import('.').TypedArrayName}`, Getter>} */
        cache,
        /** @type {(getter: Getter, name: `\$${import('.').TypedArrayName}`) => void} */
        function(getter, typedArray) {
          if (!found) {
            try {
              if ("$" + getter(value) === typedArray) {
                found = /** @type {import('.').TypedArrayName} */
                $slice(typedArray, 1);
              }
            } catch (e) {
            }
          }
        }
      );
      return found;
    };
    var trySlices = function tryAllSlices(value) {
      var found = false;
      forEach(
        /** @type {Record<`\$${import('.').TypedArrayName}`, Getter>} */
        cache,
        /** @type {(getter: Getter, name: `\$${import('.').TypedArrayName}`) => void} */
        function(getter, name) {
          if (!found) {
            try {
              getter(value);
              found = /** @type {import('.').TypedArrayName} */
              $slice(name, 1);
            } catch (e) {
            }
          }
        }
      );
      return found;
    };
    module.exports = function whichTypedArray(value) {
      if (!value || typeof value !== "object") {
        return false;
      }
      if (!hasToStringTag) {
        var tag = $slice($toString(value), 8, -1);
        if ($indexOf(typedArrays, tag) > -1) {
          return tag;
        }
        if (tag !== "Object") {
          return false;
        }
        return trySlices(value);
      }
      if (!gOPD) {
        return null;
      }
      return tryTypedArrays(value);
    };
  }
});

// node_modules/is-typed-array/index.js
var require_is_typed_array = __commonJS({
  "node_modules/is-typed-array/index.js"(exports, module) {
    "use strict";
    var whichTypedArray = require_which_typed_array();
    module.exports = function isTypedArray(value) {
      return !!whichTypedArray(value);
    };
  }
});

// node_modules/typed-array-buffer/index.js
var require_typed_array_buffer = __commonJS({
  "node_modules/typed-array-buffer/index.js"(exports, module) {
    "use strict";
    var $TypeError = require_type();
    var callBound = require_call_bound();
    var $typedArrayBuffer = callBound("TypedArray.prototype.buffer", true);
    var isTypedArray = require_is_typed_array();
    module.exports = $typedArrayBuffer || function typedArrayBuffer(x) {
      if (!isTypedArray(x)) {
        throw new $TypeError("Not a Typed Array");
      }
      return x.buffer;
    };
  }
});

// node_modules/to-buffer/index.js
var require_to_buffer = __commonJS({
  "node_modules/to-buffer/index.js"(exports, module) {
    "use strict";
    var Buffer4 = require_safe_buffer().Buffer;
    var isArray = require_isarray();
    var typedArrayBuffer = require_typed_array_buffer();
    var isView = ArrayBuffer.isView || function isView2(obj) {
      try {
        typedArrayBuffer(obj);
        return true;
      } catch (e) {
        return false;
      }
    };
    var useUint8Array = typeof Uint8Array !== "undefined";
    var useArrayBuffer = typeof ArrayBuffer !== "undefined" && typeof Uint8Array !== "undefined";
    var useFromArrayBuffer = useArrayBuffer && (Buffer4.prototype instanceof Uint8Array || Buffer4.TYPED_ARRAY_SUPPORT);
    module.exports = function toBuffer(data, encoding) {
      if (Buffer4.isBuffer(data)) {
        if (data.constructor && !("isBuffer" in data)) {
          return Buffer4.from(data);
        }
        return data;
      }
      if (typeof data === "string") {
        return Buffer4.from(data, encoding);
      }
      if (useArrayBuffer && isView(data)) {
        if (data.byteLength === 0) {
          return Buffer4.alloc(0);
        }
        if (useFromArrayBuffer) {
          var res = Buffer4.from(data.buffer, data.byteOffset, data.byteLength);
          if (res.byteLength === data.byteLength) {
            return res;
          }
        }
        var uint8 = data instanceof Uint8Array ? data : new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        var result = Buffer4.from(uint8);
        if (result.length === data.byteLength) {
          return result;
        }
      }
      if (useUint8Array && data instanceof Uint8Array) {
        return Buffer4.from(data);
      }
      var isArr = isArray(data);
      if (isArr) {
        for (var i = 0; i < data.length; i += 1) {
          var x = data[i];
          if (typeof x !== "number" || x < 0 || x > 255 || ~~x !== x) {
            throw new RangeError("Array items must be numbers in the range 0-255.");
          }
        }
      }
      if (isArr || Buffer4.isBuffer(data) && data.constructor && typeof data.constructor.isBuffer === "function" && data.constructor.isBuffer(data)) {
        return Buffer4.from(data);
      }
      throw new TypeError('The "data" argument must be a string, an Array, a Buffer, a Uint8Array, or a DataView.');
    };
  }
});

// node_modules/sha.js/hash.js
var require_hash = __commonJS({
  "node_modules/sha.js/hash.js"(exports, module) {
    "use strict";
    var Buffer4 = require_safe_buffer().Buffer;
    var toBuffer = require_to_buffer();
    function Hash(blockSize, finalSize) {
      this._block = Buffer4.alloc(blockSize);
      this._finalSize = finalSize;
      this._blockSize = blockSize;
      this._len = 0;
    }
    Hash.prototype.update = function(data, enc) {
      data = toBuffer(data, enc || "utf8");
      var block = this._block;
      var blockSize = this._blockSize;
      var length = data.length;
      var accum = this._len;
      for (var offset = 0; offset < length; ) {
        var assigned = accum % blockSize;
        var remainder = Math.min(length - offset, blockSize - assigned);
        for (var i = 0; i < remainder; i++) {
          block[assigned + i] = data[offset + i];
        }
        accum += remainder;
        offset += remainder;
        if (accum % blockSize === 0) {
          this._update(block);
        }
      }
      this._len += length;
      return this;
    };
    Hash.prototype.digest = function(enc) {
      var rem = this._len % this._blockSize;
      this._block[rem] = 128;
      this._block.fill(0, rem + 1);
      if (rem >= this._finalSize) {
        this._update(this._block);
        this._block.fill(0);
      }
      var bits = this._len * 8;
      if (bits <= 4294967295) {
        this._block.writeUInt32BE(bits, this._blockSize - 4);
      } else {
        var lowBits = (bits & 4294967295) >>> 0;
        var highBits = (bits - lowBits) / 4294967296;
        this._block.writeUInt32BE(highBits, this._blockSize - 8);
        this._block.writeUInt32BE(lowBits, this._blockSize - 4);
      }
      this._update(this._block);
      var hash = this._hash();
      return enc ? hash.toString(enc) : hash;
    };
    Hash.prototype._update = function() {
      throw new Error("_update must be implemented by subclass");
    };
    module.exports = Hash;
  }
});

// node_modules/sha.js/sha1.js
var require_sha1 = __commonJS({
  "node_modules/sha.js/sha1.js"(exports, module) {
    "use strict";
    var inherits = require_inherits();
    var Hash = require_hash();
    var Buffer4 = require_safe_buffer().Buffer;
    var K = [
      1518500249,
      1859775393,
      2400959708 | 0,
      3395469782 | 0
    ];
    var W = new Array(80);
    function Sha1() {
      this.init();
      this._w = W;
      Hash.call(this, 64, 56);
    }
    inherits(Sha1, Hash);
    Sha1.prototype.init = function() {
      this._a = 1732584193;
      this._b = 4023233417;
      this._c = 2562383102;
      this._d = 271733878;
      this._e = 3285377520;
      return this;
    };
    function rotl1(num) {
      return num << 1 | num >>> 31;
    }
    function rotl5(num) {
      return num << 5 | num >>> 27;
    }
    function rotl30(num) {
      return num << 30 | num >>> 2;
    }
    function ft(s, b, c, d) {
      if (s === 0) {
        return b & c | ~b & d;
      }
      if (s === 2) {
        return b & c | b & d | c & d;
      }
      return b ^ c ^ d;
    }
    Sha1.prototype._update = function(M) {
      var w = this._w;
      var a = this._a | 0;
      var b = this._b | 0;
      var c = this._c | 0;
      var d = this._d | 0;
      var e = this._e | 0;
      for (var i = 0; i < 16; ++i) {
        w[i] = M.readInt32BE(i * 4);
      }
      for (; i < 80; ++i) {
        w[i] = rotl1(w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]);
      }
      for (var j = 0; j < 80; ++j) {
        var s = ~~(j / 20);
        var t = rotl5(a) + ft(s, b, c, d) + e + w[j] + K[s] | 0;
        e = d;
        d = c;
        c = rotl30(b);
        b = a;
        a = t;
      }
      this._a = a + this._a | 0;
      this._b = b + this._b | 0;
      this._c = c + this._c | 0;
      this._d = d + this._d | 0;
      this._e = e + this._e | 0;
    };
    Sha1.prototype._hash = function() {
      var H = Buffer4.allocUnsafe(20);
      H.writeInt32BE(this._a | 0, 0);
      H.writeInt32BE(this._b | 0, 4);
      H.writeInt32BE(this._c | 0, 8);
      H.writeInt32BE(this._d | 0, 12);
      H.writeInt32BE(this._e | 0, 16);
      return H;
    };
    module.exports = Sha1;
  }
});

// node_modules/crc-32/crc32.js
var require_crc32 = __commonJS({
  "node_modules/crc-32/crc32.js"(exports) {
    var CRC32;
    (function(factory) {
      if (typeof DO_NOT_EXPORT_CRC === "undefined") {
        if ("object" === typeof exports) {
          factory(exports);
        } else if ("function" === typeof define && define.amd) {
          define(function() {
            var module2 = {};
            factory(module2);
            return module2;
          });
        } else {
          factory(CRC32 = {});
        }
      } else {
        factory(CRC32 = {});
      }
    })(function(CRC322) {
      CRC322.version = "1.2.2";
      function signed_crc_table() {
        var c = 0, table = new Array(256);
        for (var n = 0; n != 256; ++n) {
          c = n;
          c = c & 1 ? -306674912 ^ c >>> 1 : c >>> 1;
          c = c & 1 ? -306674912 ^ c >>> 1 : c >>> 1;
          c = c & 1 ? -306674912 ^ c >>> 1 : c >>> 1;
          c = c & 1 ? -306674912 ^ c >>> 1 : c >>> 1;
          c = c & 1 ? -306674912 ^ c >>> 1 : c >>> 1;
          c = c & 1 ? -306674912 ^ c >>> 1 : c >>> 1;
          c = c & 1 ? -306674912 ^ c >>> 1 : c >>> 1;
          c = c & 1 ? -306674912 ^ c >>> 1 : c >>> 1;
          table[n] = c;
        }
        return typeof Int32Array !== "undefined" ? new Int32Array(table) : table;
      }
      var T0 = signed_crc_table();
      function slice_by_16_tables(T) {
        var c = 0, v = 0, n = 0, table = typeof Int32Array !== "undefined" ? new Int32Array(4096) : new Array(4096);
        for (n = 0; n != 256; ++n) table[n] = T[n];
        for (n = 0; n != 256; ++n) {
          v = T[n];
          for (c = 256 + n; c < 4096; c += 256) v = table[c] = v >>> 8 ^ T[v & 255];
        }
        var out = [];
        for (n = 1; n != 16; ++n) out[n - 1] = typeof Int32Array !== "undefined" ? table.subarray(n * 256, n * 256 + 256) : table.slice(n * 256, n * 256 + 256);
        return out;
      }
      var TT = slice_by_16_tables(T0);
      var T1 = TT[0], T2 = TT[1], T3 = TT[2], T4 = TT[3], T5 = TT[4];
      var T6 = TT[5], T7 = TT[6], T8 = TT[7], T9 = TT[8], Ta = TT[9];
      var Tb = TT[10], Tc = TT[11], Td = TT[12], Te = TT[13], Tf = TT[14];
      function crc32_bstr(bstr, seed) {
        var C = seed ^ -1;
        for (var i = 0, L = bstr.length; i < L; ) C = C >>> 8 ^ T0[(C ^ bstr.charCodeAt(i++)) & 255];
        return ~C;
      }
      function crc32_buf(B, seed) {
        var C = seed ^ -1, L = B.length - 15, i = 0;
        for (; i < L; ) C = Tf[B[i++] ^ C & 255] ^ Te[B[i++] ^ C >> 8 & 255] ^ Td[B[i++] ^ C >> 16 & 255] ^ Tc[B[i++] ^ C >>> 24] ^ Tb[B[i++]] ^ Ta[B[i++]] ^ T9[B[i++]] ^ T8[B[i++]] ^ T7[B[i++]] ^ T6[B[i++]] ^ T5[B[i++]] ^ T4[B[i++]] ^ T3[B[i++]] ^ T2[B[i++]] ^ T1[B[i++]] ^ T0[B[i++]];
        L += 15;
        while (i < L) C = C >>> 8 ^ T0[(C ^ B[i++]) & 255];
        return ~C;
      }
      function crc32_str(str, seed) {
        var C = seed ^ -1;
        for (var i = 0, L = str.length, c = 0, d = 0; i < L; ) {
          c = str.charCodeAt(i++);
          if (c < 128) {
            C = C >>> 8 ^ T0[(C ^ c) & 255];
          } else if (c < 2048) {
            C = C >>> 8 ^ T0[(C ^ (192 | c >> 6 & 31)) & 255];
            C = C >>> 8 ^ T0[(C ^ (128 | c & 63)) & 255];
          } else if (c >= 55296 && c < 57344) {
            c = (c & 1023) + 64;
            d = str.charCodeAt(i++) & 1023;
            C = C >>> 8 ^ T0[(C ^ (240 | c >> 8 & 7)) & 255];
            C = C >>> 8 ^ T0[(C ^ (128 | c >> 2 & 63)) & 255];
            C = C >>> 8 ^ T0[(C ^ (128 | d >> 6 & 15 | (c & 3) << 4)) & 255];
            C = C >>> 8 ^ T0[(C ^ (128 | d & 63)) & 255];
          } else {
            C = C >>> 8 ^ T0[(C ^ (224 | c >> 12 & 15)) & 255];
            C = C >>> 8 ^ T0[(C ^ (128 | c >> 6 & 63)) & 255];
            C = C >>> 8 ^ T0[(C ^ (128 | c & 63)) & 255];
          }
        }
        return ~C;
      }
      CRC322.table = T0;
      CRC322.bstr = crc32_bstr;
      CRC322.buf = crc32_buf;
      CRC322.str = crc32_str;
    });
  }
});

// node_modules/pako/lib/utils/common.js
var require_common = __commonJS({
  "node_modules/pako/lib/utils/common.js"(exports) {
    "use strict";
    var TYPED_OK = typeof Uint8Array !== "undefined" && typeof Uint16Array !== "undefined" && typeof Int32Array !== "undefined";
    function _has(obj, key) {
      return Object.prototype.hasOwnProperty.call(obj, key);
    }
    exports.assign = function(obj) {
      var sources = Array.prototype.slice.call(arguments, 1);
      while (sources.length) {
        var source = sources.shift();
        if (!source) {
          continue;
        }
        if (typeof source !== "object") {
          throw new TypeError(source + "must be non-object");
        }
        for (var p in source) {
          if (_has(source, p)) {
            obj[p] = source[p];
          }
        }
      }
      return obj;
    };
    exports.shrinkBuf = function(buf, size) {
      if (buf.length === size) {
        return buf;
      }
      if (buf.subarray) {
        return buf.subarray(0, size);
      }
      buf.length = size;
      return buf;
    };
    var fnTyped = {
      arraySet: function(dest, src, src_offs, len, dest_offs) {
        if (src.subarray && dest.subarray) {
          dest.set(src.subarray(src_offs, src_offs + len), dest_offs);
          return;
        }
        for (var i = 0; i < len; i++) {
          dest[dest_offs + i] = src[src_offs + i];
        }
      },
      // Join array of chunks to single array.
      flattenChunks: function(chunks) {
        var i, l, len, pos, chunk, result;
        len = 0;
        for (i = 0, l = chunks.length; i < l; i++) {
          len += chunks[i].length;
        }
        result = new Uint8Array(len);
        pos = 0;
        for (i = 0, l = chunks.length; i < l; i++) {
          chunk = chunks[i];
          result.set(chunk, pos);
          pos += chunk.length;
        }
        return result;
      }
    };
    var fnUntyped = {
      arraySet: function(dest, src, src_offs, len, dest_offs) {
        for (var i = 0; i < len; i++) {
          dest[dest_offs + i] = src[src_offs + i];
        }
      },
      // Join array of chunks to single array.
      flattenChunks: function(chunks) {
        return [].concat.apply([], chunks);
      }
    };
    exports.setTyped = function(on) {
      if (on) {
        exports.Buf8 = Uint8Array;
        exports.Buf16 = Uint16Array;
        exports.Buf32 = Int32Array;
        exports.assign(exports, fnTyped);
      } else {
        exports.Buf8 = Array;
        exports.Buf16 = Array;
        exports.Buf32 = Array;
        exports.assign(exports, fnUntyped);
      }
    };
    exports.setTyped(TYPED_OK);
  }
});

// node_modules/pako/lib/zlib/trees.js
var require_trees = __commonJS({
  "node_modules/pako/lib/zlib/trees.js"(exports) {
    "use strict";
    var utils = require_common();
    var Z_FIXED = 4;
    var Z_BINARY = 0;
    var Z_TEXT = 1;
    var Z_UNKNOWN = 2;
    function zero(buf) {
      var len = buf.length;
      while (--len >= 0) {
        buf[len] = 0;
      }
    }
    var STORED_BLOCK = 0;
    var STATIC_TREES = 1;
    var DYN_TREES = 2;
    var MIN_MATCH = 3;
    var MAX_MATCH = 258;
    var LENGTH_CODES = 29;
    var LITERALS = 256;
    var L_CODES = LITERALS + 1 + LENGTH_CODES;
    var D_CODES = 30;
    var BL_CODES = 19;
    var HEAP_SIZE = 2 * L_CODES + 1;
    var MAX_BITS = 15;
    var Buf_size = 16;
    var MAX_BL_BITS = 7;
    var END_BLOCK = 256;
    var REP_3_6 = 16;
    var REPZ_3_10 = 17;
    var REPZ_11_138 = 18;
    var extra_lbits = (
      /* extra bits for each length code */
      [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0]
    );
    var extra_dbits = (
      /* extra bits for each distance code */
      [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13]
    );
    var extra_blbits = (
      /* extra bits for each bit length code */
      [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7]
    );
    var bl_order = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    var DIST_CODE_LEN = 512;
    var static_ltree = new Array((L_CODES + 2) * 2);
    zero(static_ltree);
    var static_dtree = new Array(D_CODES * 2);
    zero(static_dtree);
    var _dist_code = new Array(DIST_CODE_LEN);
    zero(_dist_code);
    var _length_code = new Array(MAX_MATCH - MIN_MATCH + 1);
    zero(_length_code);
    var base_length = new Array(LENGTH_CODES);
    zero(base_length);
    var base_dist = new Array(D_CODES);
    zero(base_dist);
    function StaticTreeDesc(static_tree, extra_bits, extra_base, elems, max_length) {
      this.static_tree = static_tree;
      this.extra_bits = extra_bits;
      this.extra_base = extra_base;
      this.elems = elems;
      this.max_length = max_length;
      this.has_stree = static_tree && static_tree.length;
    }
    var static_l_desc;
    var static_d_desc;
    var static_bl_desc;
    function TreeDesc(dyn_tree, stat_desc) {
      this.dyn_tree = dyn_tree;
      this.max_code = 0;
      this.stat_desc = stat_desc;
    }
    function d_code(dist) {
      return dist < 256 ? _dist_code[dist] : _dist_code[256 + (dist >>> 7)];
    }
    function put_short(s, w) {
      s.pending_buf[s.pending++] = w & 255;
      s.pending_buf[s.pending++] = w >>> 8 & 255;
    }
    function send_bits(s, value, length) {
      if (s.bi_valid > Buf_size - length) {
        s.bi_buf |= value << s.bi_valid & 65535;
        put_short(s, s.bi_buf);
        s.bi_buf = value >> Buf_size - s.bi_valid;
        s.bi_valid += length - Buf_size;
      } else {
        s.bi_buf |= value << s.bi_valid & 65535;
        s.bi_valid += length;
      }
    }
    function send_code(s, c, tree) {
      send_bits(
        s,
        tree[c * 2],
        tree[c * 2 + 1]
        /*.Len*/
      );
    }
    function bi_reverse(code, len) {
      var res = 0;
      do {
        res |= code & 1;
        code >>>= 1;
        res <<= 1;
      } while (--len > 0);
      return res >>> 1;
    }
    function bi_flush(s) {
      if (s.bi_valid === 16) {
        put_short(s, s.bi_buf);
        s.bi_buf = 0;
        s.bi_valid = 0;
      } else if (s.bi_valid >= 8) {
        s.pending_buf[s.pending++] = s.bi_buf & 255;
        s.bi_buf >>= 8;
        s.bi_valid -= 8;
      }
    }
    function gen_bitlen(s, desc) {
      var tree = desc.dyn_tree;
      var max_code = desc.max_code;
      var stree = desc.stat_desc.static_tree;
      var has_stree = desc.stat_desc.has_stree;
      var extra = desc.stat_desc.extra_bits;
      var base = desc.stat_desc.extra_base;
      var max_length = desc.stat_desc.max_length;
      var h;
      var n, m;
      var bits;
      var xbits;
      var f;
      var overflow = 0;
      for (bits = 0; bits <= MAX_BITS; bits++) {
        s.bl_count[bits] = 0;
      }
      tree[s.heap[s.heap_max] * 2 + 1] = 0;
      for (h = s.heap_max + 1; h < HEAP_SIZE; h++) {
        n = s.heap[h];
        bits = tree[tree[n * 2 + 1] * 2 + 1] + 1;
        if (bits > max_length) {
          bits = max_length;
          overflow++;
        }
        tree[n * 2 + 1] = bits;
        if (n > max_code) {
          continue;
        }
        s.bl_count[bits]++;
        xbits = 0;
        if (n >= base) {
          xbits = extra[n - base];
        }
        f = tree[n * 2];
        s.opt_len += f * (bits + xbits);
        if (has_stree) {
          s.static_len += f * (stree[n * 2 + 1] + xbits);
        }
      }
      if (overflow === 0) {
        return;
      }
      do {
        bits = max_length - 1;
        while (s.bl_count[bits] === 0) {
          bits--;
        }
        s.bl_count[bits]--;
        s.bl_count[bits + 1] += 2;
        s.bl_count[max_length]--;
        overflow -= 2;
      } while (overflow > 0);
      for (bits = max_length; bits !== 0; bits--) {
        n = s.bl_count[bits];
        while (n !== 0) {
          m = s.heap[--h];
          if (m > max_code) {
            continue;
          }
          if (tree[m * 2 + 1] !== bits) {
            s.opt_len += (bits - tree[m * 2 + 1]) * tree[m * 2];
            tree[m * 2 + 1] = bits;
          }
          n--;
        }
      }
    }
    function gen_codes(tree, max_code, bl_count) {
      var next_code = new Array(MAX_BITS + 1);
      var code = 0;
      var bits;
      var n;
      for (bits = 1; bits <= MAX_BITS; bits++) {
        next_code[bits] = code = code + bl_count[bits - 1] << 1;
      }
      for (n = 0; n <= max_code; n++) {
        var len = tree[n * 2 + 1];
        if (len === 0) {
          continue;
        }
        tree[n * 2] = bi_reverse(next_code[len]++, len);
      }
    }
    function tr_static_init() {
      var n;
      var bits;
      var length;
      var code;
      var dist;
      var bl_count = new Array(MAX_BITS + 1);
      length = 0;
      for (code = 0; code < LENGTH_CODES - 1; code++) {
        base_length[code] = length;
        for (n = 0; n < 1 << extra_lbits[code]; n++) {
          _length_code[length++] = code;
        }
      }
      _length_code[length - 1] = code;
      dist = 0;
      for (code = 0; code < 16; code++) {
        base_dist[code] = dist;
        for (n = 0; n < 1 << extra_dbits[code]; n++) {
          _dist_code[dist++] = code;
        }
      }
      dist >>= 7;
      for (; code < D_CODES; code++) {
        base_dist[code] = dist << 7;
        for (n = 0; n < 1 << extra_dbits[code] - 7; n++) {
          _dist_code[256 + dist++] = code;
        }
      }
      for (bits = 0; bits <= MAX_BITS; bits++) {
        bl_count[bits] = 0;
      }
      n = 0;
      while (n <= 143) {
        static_ltree[n * 2 + 1] = 8;
        n++;
        bl_count[8]++;
      }
      while (n <= 255) {
        static_ltree[n * 2 + 1] = 9;
        n++;
        bl_count[9]++;
      }
      while (n <= 279) {
        static_ltree[n * 2 + 1] = 7;
        n++;
        bl_count[7]++;
      }
      while (n <= 287) {
        static_ltree[n * 2 + 1] = 8;
        n++;
        bl_count[8]++;
      }
      gen_codes(static_ltree, L_CODES + 1, bl_count);
      for (n = 0; n < D_CODES; n++) {
        static_dtree[n * 2 + 1] = 5;
        static_dtree[n * 2] = bi_reverse(n, 5);
      }
      static_l_desc = new StaticTreeDesc(static_ltree, extra_lbits, LITERALS + 1, L_CODES, MAX_BITS);
      static_d_desc = new StaticTreeDesc(static_dtree, extra_dbits, 0, D_CODES, MAX_BITS);
      static_bl_desc = new StaticTreeDesc(new Array(0), extra_blbits, 0, BL_CODES, MAX_BL_BITS);
    }
    function init_block(s) {
      var n;
      for (n = 0; n < L_CODES; n++) {
        s.dyn_ltree[n * 2] = 0;
      }
      for (n = 0; n < D_CODES; n++) {
        s.dyn_dtree[n * 2] = 0;
      }
      for (n = 0; n < BL_CODES; n++) {
        s.bl_tree[n * 2] = 0;
      }
      s.dyn_ltree[END_BLOCK * 2] = 1;
      s.opt_len = s.static_len = 0;
      s.last_lit = s.matches = 0;
    }
    function bi_windup(s) {
      if (s.bi_valid > 8) {
        put_short(s, s.bi_buf);
      } else if (s.bi_valid > 0) {
        s.pending_buf[s.pending++] = s.bi_buf;
      }
      s.bi_buf = 0;
      s.bi_valid = 0;
    }
    function copy_block(s, buf, len, header) {
      bi_windup(s);
      if (header) {
        put_short(s, len);
        put_short(s, ~len);
      }
      utils.arraySet(s.pending_buf, s.window, buf, len, s.pending);
      s.pending += len;
    }
    function smaller(tree, n, m, depth) {
      var _n2 = n * 2;
      var _m2 = m * 2;
      return tree[_n2] < tree[_m2] || tree[_n2] === tree[_m2] && depth[n] <= depth[m];
    }
    function pqdownheap(s, tree, k) {
      var v = s.heap[k];
      var j = k << 1;
      while (j <= s.heap_len) {
        if (j < s.heap_len && smaller(tree, s.heap[j + 1], s.heap[j], s.depth)) {
          j++;
        }
        if (smaller(tree, v, s.heap[j], s.depth)) {
          break;
        }
        s.heap[k] = s.heap[j];
        k = j;
        j <<= 1;
      }
      s.heap[k] = v;
    }
    function compress_block(s, ltree, dtree) {
      var dist;
      var lc;
      var lx = 0;
      var code;
      var extra;
      if (s.last_lit !== 0) {
        do {
          dist = s.pending_buf[s.d_buf + lx * 2] << 8 | s.pending_buf[s.d_buf + lx * 2 + 1];
          lc = s.pending_buf[s.l_buf + lx];
          lx++;
          if (dist === 0) {
            send_code(s, lc, ltree);
          } else {
            code = _length_code[lc];
            send_code(s, code + LITERALS + 1, ltree);
            extra = extra_lbits[code];
            if (extra !== 0) {
              lc -= base_length[code];
              send_bits(s, lc, extra);
            }
            dist--;
            code = d_code(dist);
            send_code(s, code, dtree);
            extra = extra_dbits[code];
            if (extra !== 0) {
              dist -= base_dist[code];
              send_bits(s, dist, extra);
            }
          }
        } while (lx < s.last_lit);
      }
      send_code(s, END_BLOCK, ltree);
    }
    function build_tree(s, desc) {
      var tree = desc.dyn_tree;
      var stree = desc.stat_desc.static_tree;
      var has_stree = desc.stat_desc.has_stree;
      var elems = desc.stat_desc.elems;
      var n, m;
      var max_code = -1;
      var node;
      s.heap_len = 0;
      s.heap_max = HEAP_SIZE;
      for (n = 0; n < elems; n++) {
        if (tree[n * 2] !== 0) {
          s.heap[++s.heap_len] = max_code = n;
          s.depth[n] = 0;
        } else {
          tree[n * 2 + 1] = 0;
        }
      }
      while (s.heap_len < 2) {
        node = s.heap[++s.heap_len] = max_code < 2 ? ++max_code : 0;
        tree[node * 2] = 1;
        s.depth[node] = 0;
        s.opt_len--;
        if (has_stree) {
          s.static_len -= stree[node * 2 + 1];
        }
      }
      desc.max_code = max_code;
      for (n = s.heap_len >> 1; n >= 1; n--) {
        pqdownheap(s, tree, n);
      }
      node = elems;
      do {
        n = s.heap[
          1
          /*SMALLEST*/
        ];
        s.heap[
          1
          /*SMALLEST*/
        ] = s.heap[s.heap_len--];
        pqdownheap(
          s,
          tree,
          1
          /*SMALLEST*/
        );
        m = s.heap[
          1
          /*SMALLEST*/
        ];
        s.heap[--s.heap_max] = n;
        s.heap[--s.heap_max] = m;
        tree[node * 2] = tree[n * 2] + tree[m * 2];
        s.depth[node] = (s.depth[n] >= s.depth[m] ? s.depth[n] : s.depth[m]) + 1;
        tree[n * 2 + 1] = tree[m * 2 + 1] = node;
        s.heap[
          1
          /*SMALLEST*/
        ] = node++;
        pqdownheap(
          s,
          tree,
          1
          /*SMALLEST*/
        );
      } while (s.heap_len >= 2);
      s.heap[--s.heap_max] = s.heap[
        1
        /*SMALLEST*/
      ];
      gen_bitlen(s, desc);
      gen_codes(tree, max_code, s.bl_count);
    }
    function scan_tree(s, tree, max_code) {
      var n;
      var prevlen = -1;
      var curlen;
      var nextlen = tree[0 * 2 + 1];
      var count = 0;
      var max_count = 7;
      var min_count = 4;
      if (nextlen === 0) {
        max_count = 138;
        min_count = 3;
      }
      tree[(max_code + 1) * 2 + 1] = 65535;
      for (n = 0; n <= max_code; n++) {
        curlen = nextlen;
        nextlen = tree[(n + 1) * 2 + 1];
        if (++count < max_count && curlen === nextlen) {
          continue;
        } else if (count < min_count) {
          s.bl_tree[curlen * 2] += count;
        } else if (curlen !== 0) {
          if (curlen !== prevlen) {
            s.bl_tree[curlen * 2]++;
          }
          s.bl_tree[REP_3_6 * 2]++;
        } else if (count <= 10) {
          s.bl_tree[REPZ_3_10 * 2]++;
        } else {
          s.bl_tree[REPZ_11_138 * 2]++;
        }
        count = 0;
        prevlen = curlen;
        if (nextlen === 0) {
          max_count = 138;
          min_count = 3;
        } else if (curlen === nextlen) {
          max_count = 6;
          min_count = 3;
        } else {
          max_count = 7;
          min_count = 4;
        }
      }
    }
    function send_tree(s, tree, max_code) {
      var n;
      var prevlen = -1;
      var curlen;
      var nextlen = tree[0 * 2 + 1];
      var count = 0;
      var max_count = 7;
      var min_count = 4;
      if (nextlen === 0) {
        max_count = 138;
        min_count = 3;
      }
      for (n = 0; n <= max_code; n++) {
        curlen = nextlen;
        nextlen = tree[(n + 1) * 2 + 1];
        if (++count < max_count && curlen === nextlen) {
          continue;
        } else if (count < min_count) {
          do {
            send_code(s, curlen, s.bl_tree);
          } while (--count !== 0);
        } else if (curlen !== 0) {
          if (curlen !== prevlen) {
            send_code(s, curlen, s.bl_tree);
            count--;
          }
          send_code(s, REP_3_6, s.bl_tree);
          send_bits(s, count - 3, 2);
        } else if (count <= 10) {
          send_code(s, REPZ_3_10, s.bl_tree);
          send_bits(s, count - 3, 3);
        } else {
          send_code(s, REPZ_11_138, s.bl_tree);
          send_bits(s, count - 11, 7);
        }
        count = 0;
        prevlen = curlen;
        if (nextlen === 0) {
          max_count = 138;
          min_count = 3;
        } else if (curlen === nextlen) {
          max_count = 6;
          min_count = 3;
        } else {
          max_count = 7;
          min_count = 4;
        }
      }
    }
    function build_bl_tree(s) {
      var max_blindex;
      scan_tree(s, s.dyn_ltree, s.l_desc.max_code);
      scan_tree(s, s.dyn_dtree, s.d_desc.max_code);
      build_tree(s, s.bl_desc);
      for (max_blindex = BL_CODES - 1; max_blindex >= 3; max_blindex--) {
        if (s.bl_tree[bl_order[max_blindex] * 2 + 1] !== 0) {
          break;
        }
      }
      s.opt_len += 3 * (max_blindex + 1) + 5 + 5 + 4;
      return max_blindex;
    }
    function send_all_trees(s, lcodes, dcodes, blcodes) {
      var rank;
      send_bits(s, lcodes - 257, 5);
      send_bits(s, dcodes - 1, 5);
      send_bits(s, blcodes - 4, 4);
      for (rank = 0; rank < blcodes; rank++) {
        send_bits(s, s.bl_tree[bl_order[rank] * 2 + 1], 3);
      }
      send_tree(s, s.dyn_ltree, lcodes - 1);
      send_tree(s, s.dyn_dtree, dcodes - 1);
    }
    function detect_data_type(s) {
      var black_mask = 4093624447;
      var n;
      for (n = 0; n <= 31; n++, black_mask >>>= 1) {
        if (black_mask & 1 && s.dyn_ltree[n * 2] !== 0) {
          return Z_BINARY;
        }
      }
      if (s.dyn_ltree[9 * 2] !== 0 || s.dyn_ltree[10 * 2] !== 0 || s.dyn_ltree[13 * 2] !== 0) {
        return Z_TEXT;
      }
      for (n = 32; n < LITERALS; n++) {
        if (s.dyn_ltree[n * 2] !== 0) {
          return Z_TEXT;
        }
      }
      return Z_BINARY;
    }
    var static_init_done = false;
    function _tr_init(s) {
      if (!static_init_done) {
        tr_static_init();
        static_init_done = true;
      }
      s.l_desc = new TreeDesc(s.dyn_ltree, static_l_desc);
      s.d_desc = new TreeDesc(s.dyn_dtree, static_d_desc);
      s.bl_desc = new TreeDesc(s.bl_tree, static_bl_desc);
      s.bi_buf = 0;
      s.bi_valid = 0;
      init_block(s);
    }
    function _tr_stored_block(s, buf, stored_len, last) {
      send_bits(s, (STORED_BLOCK << 1) + (last ? 1 : 0), 3);
      copy_block(s, buf, stored_len, true);
    }
    function _tr_align(s) {
      send_bits(s, STATIC_TREES << 1, 3);
      send_code(s, END_BLOCK, static_ltree);
      bi_flush(s);
    }
    function _tr_flush_block(s, buf, stored_len, last) {
      var opt_lenb, static_lenb;
      var max_blindex = 0;
      if (s.level > 0) {
        if (s.strm.data_type === Z_UNKNOWN) {
          s.strm.data_type = detect_data_type(s);
        }
        build_tree(s, s.l_desc);
        build_tree(s, s.d_desc);
        max_blindex = build_bl_tree(s);
        opt_lenb = s.opt_len + 3 + 7 >>> 3;
        static_lenb = s.static_len + 3 + 7 >>> 3;
        if (static_lenb <= opt_lenb) {
          opt_lenb = static_lenb;
        }
      } else {
        opt_lenb = static_lenb = stored_len + 5;
      }
      if (stored_len + 4 <= opt_lenb && buf !== -1) {
        _tr_stored_block(s, buf, stored_len, last);
      } else if (s.strategy === Z_FIXED || static_lenb === opt_lenb) {
        send_bits(s, (STATIC_TREES << 1) + (last ? 1 : 0), 3);
        compress_block(s, static_ltree, static_dtree);
      } else {
        send_bits(s, (DYN_TREES << 1) + (last ? 1 : 0), 3);
        send_all_trees(s, s.l_desc.max_code + 1, s.d_desc.max_code + 1, max_blindex + 1);
        compress_block(s, s.dyn_ltree, s.dyn_dtree);
      }
      init_block(s);
      if (last) {
        bi_windup(s);
      }
    }
    function _tr_tally(s, dist, lc) {
      s.pending_buf[s.d_buf + s.last_lit * 2] = dist >>> 8 & 255;
      s.pending_buf[s.d_buf + s.last_lit * 2 + 1] = dist & 255;
      s.pending_buf[s.l_buf + s.last_lit] = lc & 255;
      s.last_lit++;
      if (dist === 0) {
        s.dyn_ltree[lc * 2]++;
      } else {
        s.matches++;
        dist--;
        s.dyn_ltree[(_length_code[lc] + LITERALS + 1) * 2]++;
        s.dyn_dtree[d_code(dist) * 2]++;
      }
      return s.last_lit === s.lit_bufsize - 1;
    }
    exports._tr_init = _tr_init;
    exports._tr_stored_block = _tr_stored_block;
    exports._tr_flush_block = _tr_flush_block;
    exports._tr_tally = _tr_tally;
    exports._tr_align = _tr_align;
  }
});

// node_modules/pako/lib/zlib/adler32.js
var require_adler32 = __commonJS({
  "node_modules/pako/lib/zlib/adler32.js"(exports, module) {
    "use strict";
    function adler32(adler, buf, len, pos) {
      var s1 = adler & 65535 | 0, s2 = adler >>> 16 & 65535 | 0, n = 0;
      while (len !== 0) {
        n = len > 2e3 ? 2e3 : len;
        len -= n;
        do {
          s1 = s1 + buf[pos++] | 0;
          s2 = s2 + s1 | 0;
        } while (--n);
        s1 %= 65521;
        s2 %= 65521;
      }
      return s1 | s2 << 16 | 0;
    }
    module.exports = adler32;
  }
});

// node_modules/pako/lib/zlib/crc32.js
var require_crc322 = __commonJS({
  "node_modules/pako/lib/zlib/crc32.js"(exports, module) {
    "use strict";
    function makeTable() {
      var c, table = [];
      for (var n = 0; n < 256; n++) {
        c = n;
        for (var k = 0; k < 8; k++) {
          c = c & 1 ? 3988292384 ^ c >>> 1 : c >>> 1;
        }
        table[n] = c;
      }
      return table;
    }
    var crcTable = makeTable();
    function crc32(crc, buf, len, pos) {
      var t = crcTable, end = pos + len;
      crc ^= -1;
      for (var i = pos; i < end; i++) {
        crc = crc >>> 8 ^ t[(crc ^ buf[i]) & 255];
      }
      return crc ^ -1;
    }
    module.exports = crc32;
  }
});

// node_modules/pako/lib/zlib/messages.js
var require_messages = __commonJS({
  "node_modules/pako/lib/zlib/messages.js"(exports, module) {
    "use strict";
    module.exports = {
      2: "need dictionary",
      /* Z_NEED_DICT       2  */
      1: "stream end",
      /* Z_STREAM_END      1  */
      0: "",
      /* Z_OK              0  */
      "-1": "file error",
      /* Z_ERRNO         (-1) */
      "-2": "stream error",
      /* Z_STREAM_ERROR  (-2) */
      "-3": "data error",
      /* Z_DATA_ERROR    (-3) */
      "-4": "insufficient memory",
      /* Z_MEM_ERROR     (-4) */
      "-5": "buffer error",
      /* Z_BUF_ERROR     (-5) */
      "-6": "incompatible version"
      /* Z_VERSION_ERROR (-6) */
    };
  }
});

// node_modules/pako/lib/zlib/deflate.js
var require_deflate = __commonJS({
  "node_modules/pako/lib/zlib/deflate.js"(exports) {
    "use strict";
    var utils = require_common();
    var trees = require_trees();
    var adler32 = require_adler32();
    var crc32 = require_crc322();
    var msg = require_messages();
    var Z_NO_FLUSH = 0;
    var Z_PARTIAL_FLUSH = 1;
    var Z_FULL_FLUSH = 3;
    var Z_FINISH = 4;
    var Z_BLOCK = 5;
    var Z_OK = 0;
    var Z_STREAM_END = 1;
    var Z_STREAM_ERROR = -2;
    var Z_DATA_ERROR = -3;
    var Z_BUF_ERROR = -5;
    var Z_DEFAULT_COMPRESSION = -1;
    var Z_FILTERED = 1;
    var Z_HUFFMAN_ONLY = 2;
    var Z_RLE = 3;
    var Z_FIXED = 4;
    var Z_DEFAULT_STRATEGY = 0;
    var Z_UNKNOWN = 2;
    var Z_DEFLATED = 8;
    var MAX_MEM_LEVEL = 9;
    var MAX_WBITS = 15;
    var DEF_MEM_LEVEL = 8;
    var LENGTH_CODES = 29;
    var LITERALS = 256;
    var L_CODES = LITERALS + 1 + LENGTH_CODES;
    var D_CODES = 30;
    var BL_CODES = 19;
    var HEAP_SIZE = 2 * L_CODES + 1;
    var MAX_BITS = 15;
    var MIN_MATCH = 3;
    var MAX_MATCH = 258;
    var MIN_LOOKAHEAD = MAX_MATCH + MIN_MATCH + 1;
    var PRESET_DICT = 32;
    var INIT_STATE = 42;
    var EXTRA_STATE = 69;
    var NAME_STATE = 73;
    var COMMENT_STATE = 91;
    var HCRC_STATE = 103;
    var BUSY_STATE = 113;
    var FINISH_STATE = 666;
    var BS_NEED_MORE = 1;
    var BS_BLOCK_DONE = 2;
    var BS_FINISH_STARTED = 3;
    var BS_FINISH_DONE = 4;
    var OS_CODE = 3;
    function err(strm, errorCode) {
      strm.msg = msg[errorCode];
      return errorCode;
    }
    function rank(f) {
      return (f << 1) - (f > 4 ? 9 : 0);
    }
    function zero(buf) {
      var len = buf.length;
      while (--len >= 0) {
        buf[len] = 0;
      }
    }
    function flush_pending(strm) {
      var s = strm.state;
      var len = s.pending;
      if (len > strm.avail_out) {
        len = strm.avail_out;
      }
      if (len === 0) {
        return;
      }
      utils.arraySet(strm.output, s.pending_buf, s.pending_out, len, strm.next_out);
      strm.next_out += len;
      s.pending_out += len;
      strm.total_out += len;
      strm.avail_out -= len;
      s.pending -= len;
      if (s.pending === 0) {
        s.pending_out = 0;
      }
    }
    function flush_block_only(s, last) {
      trees._tr_flush_block(s, s.block_start >= 0 ? s.block_start : -1, s.strstart - s.block_start, last);
      s.block_start = s.strstart;
      flush_pending(s.strm);
    }
    function put_byte(s, b) {
      s.pending_buf[s.pending++] = b;
    }
    function putShortMSB(s, b) {
      s.pending_buf[s.pending++] = b >>> 8 & 255;
      s.pending_buf[s.pending++] = b & 255;
    }
    function read_buf(strm, buf, start, size) {
      var len = strm.avail_in;
      if (len > size) {
        len = size;
      }
      if (len === 0) {
        return 0;
      }
      strm.avail_in -= len;
      utils.arraySet(buf, strm.input, strm.next_in, len, start);
      if (strm.state.wrap === 1) {
        strm.adler = adler32(strm.adler, buf, len, start);
      } else if (strm.state.wrap === 2) {
        strm.adler = crc32(strm.adler, buf, len, start);
      }
      strm.next_in += len;
      strm.total_in += len;
      return len;
    }
    function longest_match(s, cur_match) {
      var chain_length = s.max_chain_length;
      var scan = s.strstart;
      var match;
      var len;
      var best_len = s.prev_length;
      var nice_match = s.nice_match;
      var limit = s.strstart > s.w_size - MIN_LOOKAHEAD ? s.strstart - (s.w_size - MIN_LOOKAHEAD) : 0;
      var _win = s.window;
      var wmask = s.w_mask;
      var prev = s.prev;
      var strend = s.strstart + MAX_MATCH;
      var scan_end1 = _win[scan + best_len - 1];
      var scan_end = _win[scan + best_len];
      if (s.prev_length >= s.good_match) {
        chain_length >>= 2;
      }
      if (nice_match > s.lookahead) {
        nice_match = s.lookahead;
      }
      do {
        match = cur_match;
        if (_win[match + best_len] !== scan_end || _win[match + best_len - 1] !== scan_end1 || _win[match] !== _win[scan] || _win[++match] !== _win[scan + 1]) {
          continue;
        }
        scan += 2;
        match++;
        do {
        } while (_win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && _win[++scan] === _win[++match] && scan < strend);
        len = MAX_MATCH - (strend - scan);
        scan = strend - MAX_MATCH;
        if (len > best_len) {
          s.match_start = cur_match;
          best_len = len;
          if (len >= nice_match) {
            break;
          }
          scan_end1 = _win[scan + best_len - 1];
          scan_end = _win[scan + best_len];
        }
      } while ((cur_match = prev[cur_match & wmask]) > limit && --chain_length !== 0);
      if (best_len <= s.lookahead) {
        return best_len;
      }
      return s.lookahead;
    }
    function fill_window(s) {
      var _w_size = s.w_size;
      var p, n, m, more, str;
      do {
        more = s.window_size - s.lookahead - s.strstart;
        if (s.strstart >= _w_size + (_w_size - MIN_LOOKAHEAD)) {
          utils.arraySet(s.window, s.window, _w_size, _w_size, 0);
          s.match_start -= _w_size;
          s.strstart -= _w_size;
          s.block_start -= _w_size;
          n = s.hash_size;
          p = n;
          do {
            m = s.head[--p];
            s.head[p] = m >= _w_size ? m - _w_size : 0;
          } while (--n);
          n = _w_size;
          p = n;
          do {
            m = s.prev[--p];
            s.prev[p] = m >= _w_size ? m - _w_size : 0;
          } while (--n);
          more += _w_size;
        }
        if (s.strm.avail_in === 0) {
          break;
        }
        n = read_buf(s.strm, s.window, s.strstart + s.lookahead, more);
        s.lookahead += n;
        if (s.lookahead + s.insert >= MIN_MATCH) {
          str = s.strstart - s.insert;
          s.ins_h = s.window[str];
          s.ins_h = (s.ins_h << s.hash_shift ^ s.window[str + 1]) & s.hash_mask;
          while (s.insert) {
            s.ins_h = (s.ins_h << s.hash_shift ^ s.window[str + MIN_MATCH - 1]) & s.hash_mask;
            s.prev[str & s.w_mask] = s.head[s.ins_h];
            s.head[s.ins_h] = str;
            str++;
            s.insert--;
            if (s.lookahead + s.insert < MIN_MATCH) {
              break;
            }
          }
        }
      } while (s.lookahead < MIN_LOOKAHEAD && s.strm.avail_in !== 0);
    }
    function deflate_stored(s, flush) {
      var max_block_size = 65535;
      if (max_block_size > s.pending_buf_size - 5) {
        max_block_size = s.pending_buf_size - 5;
      }
      for (; ; ) {
        if (s.lookahead <= 1) {
          fill_window(s);
          if (s.lookahead === 0 && flush === Z_NO_FLUSH) {
            return BS_NEED_MORE;
          }
          if (s.lookahead === 0) {
            break;
          }
        }
        s.strstart += s.lookahead;
        s.lookahead = 0;
        var max_start = s.block_start + max_block_size;
        if (s.strstart === 0 || s.strstart >= max_start) {
          s.lookahead = s.strstart - max_start;
          s.strstart = max_start;
          flush_block_only(s, false);
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        }
        if (s.strstart - s.block_start >= s.w_size - MIN_LOOKAHEAD) {
          flush_block_only(s, false);
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        }
      }
      s.insert = 0;
      if (flush === Z_FINISH) {
        flush_block_only(s, true);
        if (s.strm.avail_out === 0) {
          return BS_FINISH_STARTED;
        }
        return BS_FINISH_DONE;
      }
      if (s.strstart > s.block_start) {
        flush_block_only(s, false);
        if (s.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      return BS_NEED_MORE;
    }
    function deflate_fast(s, flush) {
      var hash_head;
      var bflush;
      for (; ; ) {
        if (s.lookahead < MIN_LOOKAHEAD) {
          fill_window(s);
          if (s.lookahead < MIN_LOOKAHEAD && flush === Z_NO_FLUSH) {
            return BS_NEED_MORE;
          }
          if (s.lookahead === 0) {
            break;
          }
        }
        hash_head = 0;
        if (s.lookahead >= MIN_MATCH) {
          s.ins_h = (s.ins_h << s.hash_shift ^ s.window[s.strstart + MIN_MATCH - 1]) & s.hash_mask;
          hash_head = s.prev[s.strstart & s.w_mask] = s.head[s.ins_h];
          s.head[s.ins_h] = s.strstart;
        }
        if (hash_head !== 0 && s.strstart - hash_head <= s.w_size - MIN_LOOKAHEAD) {
          s.match_length = longest_match(s, hash_head);
        }
        if (s.match_length >= MIN_MATCH) {
          bflush = trees._tr_tally(s, s.strstart - s.match_start, s.match_length - MIN_MATCH);
          s.lookahead -= s.match_length;
          if (s.match_length <= s.max_lazy_match && s.lookahead >= MIN_MATCH) {
            s.match_length--;
            do {
              s.strstart++;
              s.ins_h = (s.ins_h << s.hash_shift ^ s.window[s.strstart + MIN_MATCH - 1]) & s.hash_mask;
              hash_head = s.prev[s.strstart & s.w_mask] = s.head[s.ins_h];
              s.head[s.ins_h] = s.strstart;
            } while (--s.match_length !== 0);
            s.strstart++;
          } else {
            s.strstart += s.match_length;
            s.match_length = 0;
            s.ins_h = s.window[s.strstart];
            s.ins_h = (s.ins_h << s.hash_shift ^ s.window[s.strstart + 1]) & s.hash_mask;
          }
        } else {
          bflush = trees._tr_tally(s, 0, s.window[s.strstart]);
          s.lookahead--;
          s.strstart++;
        }
        if (bflush) {
          flush_block_only(s, false);
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        }
      }
      s.insert = s.strstart < MIN_MATCH - 1 ? s.strstart : MIN_MATCH - 1;
      if (flush === Z_FINISH) {
        flush_block_only(s, true);
        if (s.strm.avail_out === 0) {
          return BS_FINISH_STARTED;
        }
        return BS_FINISH_DONE;
      }
      if (s.last_lit) {
        flush_block_only(s, false);
        if (s.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      return BS_BLOCK_DONE;
    }
    function deflate_slow(s, flush) {
      var hash_head;
      var bflush;
      var max_insert;
      for (; ; ) {
        if (s.lookahead < MIN_LOOKAHEAD) {
          fill_window(s);
          if (s.lookahead < MIN_LOOKAHEAD && flush === Z_NO_FLUSH) {
            return BS_NEED_MORE;
          }
          if (s.lookahead === 0) {
            break;
          }
        }
        hash_head = 0;
        if (s.lookahead >= MIN_MATCH) {
          s.ins_h = (s.ins_h << s.hash_shift ^ s.window[s.strstart + MIN_MATCH - 1]) & s.hash_mask;
          hash_head = s.prev[s.strstart & s.w_mask] = s.head[s.ins_h];
          s.head[s.ins_h] = s.strstart;
        }
        s.prev_length = s.match_length;
        s.prev_match = s.match_start;
        s.match_length = MIN_MATCH - 1;
        if (hash_head !== 0 && s.prev_length < s.max_lazy_match && s.strstart - hash_head <= s.w_size - MIN_LOOKAHEAD) {
          s.match_length = longest_match(s, hash_head);
          if (s.match_length <= 5 && (s.strategy === Z_FILTERED || s.match_length === MIN_MATCH && s.strstart - s.match_start > 4096)) {
            s.match_length = MIN_MATCH - 1;
          }
        }
        if (s.prev_length >= MIN_MATCH && s.match_length <= s.prev_length) {
          max_insert = s.strstart + s.lookahead - MIN_MATCH;
          bflush = trees._tr_tally(s, s.strstart - 1 - s.prev_match, s.prev_length - MIN_MATCH);
          s.lookahead -= s.prev_length - 1;
          s.prev_length -= 2;
          do {
            if (++s.strstart <= max_insert) {
              s.ins_h = (s.ins_h << s.hash_shift ^ s.window[s.strstart + MIN_MATCH - 1]) & s.hash_mask;
              hash_head = s.prev[s.strstart & s.w_mask] = s.head[s.ins_h];
              s.head[s.ins_h] = s.strstart;
            }
          } while (--s.prev_length !== 0);
          s.match_available = 0;
          s.match_length = MIN_MATCH - 1;
          s.strstart++;
          if (bflush) {
            flush_block_only(s, false);
            if (s.strm.avail_out === 0) {
              return BS_NEED_MORE;
            }
          }
        } else if (s.match_available) {
          bflush = trees._tr_tally(s, 0, s.window[s.strstart - 1]);
          if (bflush) {
            flush_block_only(s, false);
          }
          s.strstart++;
          s.lookahead--;
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        } else {
          s.match_available = 1;
          s.strstart++;
          s.lookahead--;
        }
      }
      if (s.match_available) {
        bflush = trees._tr_tally(s, 0, s.window[s.strstart - 1]);
        s.match_available = 0;
      }
      s.insert = s.strstart < MIN_MATCH - 1 ? s.strstart : MIN_MATCH - 1;
      if (flush === Z_FINISH) {
        flush_block_only(s, true);
        if (s.strm.avail_out === 0) {
          return BS_FINISH_STARTED;
        }
        return BS_FINISH_DONE;
      }
      if (s.last_lit) {
        flush_block_only(s, false);
        if (s.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      return BS_BLOCK_DONE;
    }
    function deflate_rle(s, flush) {
      var bflush;
      var prev;
      var scan, strend;
      var _win = s.window;
      for (; ; ) {
        if (s.lookahead <= MAX_MATCH) {
          fill_window(s);
          if (s.lookahead <= MAX_MATCH && flush === Z_NO_FLUSH) {
            return BS_NEED_MORE;
          }
          if (s.lookahead === 0) {
            break;
          }
        }
        s.match_length = 0;
        if (s.lookahead >= MIN_MATCH && s.strstart > 0) {
          scan = s.strstart - 1;
          prev = _win[scan];
          if (prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan]) {
            strend = s.strstart + MAX_MATCH;
            do {
            } while (prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && prev === _win[++scan] && scan < strend);
            s.match_length = MAX_MATCH - (strend - scan);
            if (s.match_length > s.lookahead) {
              s.match_length = s.lookahead;
            }
          }
        }
        if (s.match_length >= MIN_MATCH) {
          bflush = trees._tr_tally(s, 1, s.match_length - MIN_MATCH);
          s.lookahead -= s.match_length;
          s.strstart += s.match_length;
          s.match_length = 0;
        } else {
          bflush = trees._tr_tally(s, 0, s.window[s.strstart]);
          s.lookahead--;
          s.strstart++;
        }
        if (bflush) {
          flush_block_only(s, false);
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        }
      }
      s.insert = 0;
      if (flush === Z_FINISH) {
        flush_block_only(s, true);
        if (s.strm.avail_out === 0) {
          return BS_FINISH_STARTED;
        }
        return BS_FINISH_DONE;
      }
      if (s.last_lit) {
        flush_block_only(s, false);
        if (s.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      return BS_BLOCK_DONE;
    }
    function deflate_huff(s, flush) {
      var bflush;
      for (; ; ) {
        if (s.lookahead === 0) {
          fill_window(s);
          if (s.lookahead === 0) {
            if (flush === Z_NO_FLUSH) {
              return BS_NEED_MORE;
            }
            break;
          }
        }
        s.match_length = 0;
        bflush = trees._tr_tally(s, 0, s.window[s.strstart]);
        s.lookahead--;
        s.strstart++;
        if (bflush) {
          flush_block_only(s, false);
          if (s.strm.avail_out === 0) {
            return BS_NEED_MORE;
          }
        }
      }
      s.insert = 0;
      if (flush === Z_FINISH) {
        flush_block_only(s, true);
        if (s.strm.avail_out === 0) {
          return BS_FINISH_STARTED;
        }
        return BS_FINISH_DONE;
      }
      if (s.last_lit) {
        flush_block_only(s, false);
        if (s.strm.avail_out === 0) {
          return BS_NEED_MORE;
        }
      }
      return BS_BLOCK_DONE;
    }
    function Config(good_length, max_lazy, nice_length, max_chain, func) {
      this.good_length = good_length;
      this.max_lazy = max_lazy;
      this.nice_length = nice_length;
      this.max_chain = max_chain;
      this.func = func;
    }
    var configuration_table;
    configuration_table = [
      /*      good lazy nice chain */
      new Config(0, 0, 0, 0, deflate_stored),
      /* 0 store only */
      new Config(4, 4, 8, 4, deflate_fast),
      /* 1 max speed, no lazy matches */
      new Config(4, 5, 16, 8, deflate_fast),
      /* 2 */
      new Config(4, 6, 32, 32, deflate_fast),
      /* 3 */
      new Config(4, 4, 16, 16, deflate_slow),
      /* 4 lazy matches */
      new Config(8, 16, 32, 32, deflate_slow),
      /* 5 */
      new Config(8, 16, 128, 128, deflate_slow),
      /* 6 */
      new Config(8, 32, 128, 256, deflate_slow),
      /* 7 */
      new Config(32, 128, 258, 1024, deflate_slow),
      /* 8 */
      new Config(32, 258, 258, 4096, deflate_slow)
      /* 9 max compression */
    ];
    function lm_init(s) {
      s.window_size = 2 * s.w_size;
      zero(s.head);
      s.max_lazy_match = configuration_table[s.level].max_lazy;
      s.good_match = configuration_table[s.level].good_length;
      s.nice_match = configuration_table[s.level].nice_length;
      s.max_chain_length = configuration_table[s.level].max_chain;
      s.strstart = 0;
      s.block_start = 0;
      s.lookahead = 0;
      s.insert = 0;
      s.match_length = s.prev_length = MIN_MATCH - 1;
      s.match_available = 0;
      s.ins_h = 0;
    }
    function DeflateState() {
      this.strm = null;
      this.status = 0;
      this.pending_buf = null;
      this.pending_buf_size = 0;
      this.pending_out = 0;
      this.pending = 0;
      this.wrap = 0;
      this.gzhead = null;
      this.gzindex = 0;
      this.method = Z_DEFLATED;
      this.last_flush = -1;
      this.w_size = 0;
      this.w_bits = 0;
      this.w_mask = 0;
      this.window = null;
      this.window_size = 0;
      this.prev = null;
      this.head = null;
      this.ins_h = 0;
      this.hash_size = 0;
      this.hash_bits = 0;
      this.hash_mask = 0;
      this.hash_shift = 0;
      this.block_start = 0;
      this.match_length = 0;
      this.prev_match = 0;
      this.match_available = 0;
      this.strstart = 0;
      this.match_start = 0;
      this.lookahead = 0;
      this.prev_length = 0;
      this.max_chain_length = 0;
      this.max_lazy_match = 0;
      this.level = 0;
      this.strategy = 0;
      this.good_match = 0;
      this.nice_match = 0;
      this.dyn_ltree = new utils.Buf16(HEAP_SIZE * 2);
      this.dyn_dtree = new utils.Buf16((2 * D_CODES + 1) * 2);
      this.bl_tree = new utils.Buf16((2 * BL_CODES + 1) * 2);
      zero(this.dyn_ltree);
      zero(this.dyn_dtree);
      zero(this.bl_tree);
      this.l_desc = null;
      this.d_desc = null;
      this.bl_desc = null;
      this.bl_count = new utils.Buf16(MAX_BITS + 1);
      this.heap = new utils.Buf16(2 * L_CODES + 1);
      zero(this.heap);
      this.heap_len = 0;
      this.heap_max = 0;
      this.depth = new utils.Buf16(2 * L_CODES + 1);
      zero(this.depth);
      this.l_buf = 0;
      this.lit_bufsize = 0;
      this.last_lit = 0;
      this.d_buf = 0;
      this.opt_len = 0;
      this.static_len = 0;
      this.matches = 0;
      this.insert = 0;
      this.bi_buf = 0;
      this.bi_valid = 0;
    }
    function deflateResetKeep(strm) {
      var s;
      if (!strm || !strm.state) {
        return err(strm, Z_STREAM_ERROR);
      }
      strm.total_in = strm.total_out = 0;
      strm.data_type = Z_UNKNOWN;
      s = strm.state;
      s.pending = 0;
      s.pending_out = 0;
      if (s.wrap < 0) {
        s.wrap = -s.wrap;
      }
      s.status = s.wrap ? INIT_STATE : BUSY_STATE;
      strm.adler = s.wrap === 2 ? 0 : 1;
      s.last_flush = Z_NO_FLUSH;
      trees._tr_init(s);
      return Z_OK;
    }
    function deflateReset(strm) {
      var ret = deflateResetKeep(strm);
      if (ret === Z_OK) {
        lm_init(strm.state);
      }
      return ret;
    }
    function deflateSetHeader(strm, head) {
      if (!strm || !strm.state) {
        return Z_STREAM_ERROR;
      }
      if (strm.state.wrap !== 2) {
        return Z_STREAM_ERROR;
      }
      strm.state.gzhead = head;
      return Z_OK;
    }
    function deflateInit2(strm, level, method, windowBits, memLevel, strategy) {
      if (!strm) {
        return Z_STREAM_ERROR;
      }
      var wrap = 1;
      if (level === Z_DEFAULT_COMPRESSION) {
        level = 6;
      }
      if (windowBits < 0) {
        wrap = 0;
        windowBits = -windowBits;
      } else if (windowBits > 15) {
        wrap = 2;
        windowBits -= 16;
      }
      if (memLevel < 1 || memLevel > MAX_MEM_LEVEL || method !== Z_DEFLATED || windowBits < 8 || windowBits > 15 || level < 0 || level > 9 || strategy < 0 || strategy > Z_FIXED) {
        return err(strm, Z_STREAM_ERROR);
      }
      if (windowBits === 8) {
        windowBits = 9;
      }
      var s = new DeflateState();
      strm.state = s;
      s.strm = strm;
      s.wrap = wrap;
      s.gzhead = null;
      s.w_bits = windowBits;
      s.w_size = 1 << s.w_bits;
      s.w_mask = s.w_size - 1;
      s.hash_bits = memLevel + 7;
      s.hash_size = 1 << s.hash_bits;
      s.hash_mask = s.hash_size - 1;
      s.hash_shift = ~~((s.hash_bits + MIN_MATCH - 1) / MIN_MATCH);
      s.window = new utils.Buf8(s.w_size * 2);
      s.head = new utils.Buf16(s.hash_size);
      s.prev = new utils.Buf16(s.w_size);
      s.lit_bufsize = 1 << memLevel + 6;
      s.pending_buf_size = s.lit_bufsize * 4;
      s.pending_buf = new utils.Buf8(s.pending_buf_size);
      s.d_buf = 1 * s.lit_bufsize;
      s.l_buf = (1 + 2) * s.lit_bufsize;
      s.level = level;
      s.strategy = strategy;
      s.method = method;
      return deflateReset(strm);
    }
    function deflateInit(strm, level) {
      return deflateInit2(strm, level, Z_DEFLATED, MAX_WBITS, DEF_MEM_LEVEL, Z_DEFAULT_STRATEGY);
    }
    function deflate(strm, flush) {
      var old_flush, s;
      var beg, val;
      if (!strm || !strm.state || flush > Z_BLOCK || flush < 0) {
        return strm ? err(strm, Z_STREAM_ERROR) : Z_STREAM_ERROR;
      }
      s = strm.state;
      if (!strm.output || !strm.input && strm.avail_in !== 0 || s.status === FINISH_STATE && flush !== Z_FINISH) {
        return err(strm, strm.avail_out === 0 ? Z_BUF_ERROR : Z_STREAM_ERROR);
      }
      s.strm = strm;
      old_flush = s.last_flush;
      s.last_flush = flush;
      if (s.status === INIT_STATE) {
        if (s.wrap === 2) {
          strm.adler = 0;
          put_byte(s, 31);
          put_byte(s, 139);
          put_byte(s, 8);
          if (!s.gzhead) {
            put_byte(s, 0);
            put_byte(s, 0);
            put_byte(s, 0);
            put_byte(s, 0);
            put_byte(s, 0);
            put_byte(s, s.level === 9 ? 2 : s.strategy >= Z_HUFFMAN_ONLY || s.level < 2 ? 4 : 0);
            put_byte(s, OS_CODE);
            s.status = BUSY_STATE;
          } else {
            put_byte(
              s,
              (s.gzhead.text ? 1 : 0) + (s.gzhead.hcrc ? 2 : 0) + (!s.gzhead.extra ? 0 : 4) + (!s.gzhead.name ? 0 : 8) + (!s.gzhead.comment ? 0 : 16)
            );
            put_byte(s, s.gzhead.time & 255);
            put_byte(s, s.gzhead.time >> 8 & 255);
            put_byte(s, s.gzhead.time >> 16 & 255);
            put_byte(s, s.gzhead.time >> 24 & 255);
            put_byte(s, s.level === 9 ? 2 : s.strategy >= Z_HUFFMAN_ONLY || s.level < 2 ? 4 : 0);
            put_byte(s, s.gzhead.os & 255);
            if (s.gzhead.extra && s.gzhead.extra.length) {
              put_byte(s, s.gzhead.extra.length & 255);
              put_byte(s, s.gzhead.extra.length >> 8 & 255);
            }
            if (s.gzhead.hcrc) {
              strm.adler = crc32(strm.adler, s.pending_buf, s.pending, 0);
            }
            s.gzindex = 0;
            s.status = EXTRA_STATE;
          }
        } else {
          var header = Z_DEFLATED + (s.w_bits - 8 << 4) << 8;
          var level_flags = -1;
          if (s.strategy >= Z_HUFFMAN_ONLY || s.level < 2) {
            level_flags = 0;
          } else if (s.level < 6) {
            level_flags = 1;
          } else if (s.level === 6) {
            level_flags = 2;
          } else {
            level_flags = 3;
          }
          header |= level_flags << 6;
          if (s.strstart !== 0) {
            header |= PRESET_DICT;
          }
          header += 31 - header % 31;
          s.status = BUSY_STATE;
          putShortMSB(s, header);
          if (s.strstart !== 0) {
            putShortMSB(s, strm.adler >>> 16);
            putShortMSB(s, strm.adler & 65535);
          }
          strm.adler = 1;
        }
      }
      if (s.status === EXTRA_STATE) {
        if (s.gzhead.extra) {
          beg = s.pending;
          while (s.gzindex < (s.gzhead.extra.length & 65535)) {
            if (s.pending === s.pending_buf_size) {
              if (s.gzhead.hcrc && s.pending > beg) {
                strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
              }
              flush_pending(strm);
              beg = s.pending;
              if (s.pending === s.pending_buf_size) {
                break;
              }
            }
            put_byte(s, s.gzhead.extra[s.gzindex] & 255);
            s.gzindex++;
          }
          if (s.gzhead.hcrc && s.pending > beg) {
            strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
          }
          if (s.gzindex === s.gzhead.extra.length) {
            s.gzindex = 0;
            s.status = NAME_STATE;
          }
        } else {
          s.status = NAME_STATE;
        }
      }
      if (s.status === NAME_STATE) {
        if (s.gzhead.name) {
          beg = s.pending;
          do {
            if (s.pending === s.pending_buf_size) {
              if (s.gzhead.hcrc && s.pending > beg) {
                strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
              }
              flush_pending(strm);
              beg = s.pending;
              if (s.pending === s.pending_buf_size) {
                val = 1;
                break;
              }
            }
            if (s.gzindex < s.gzhead.name.length) {
              val = s.gzhead.name.charCodeAt(s.gzindex++) & 255;
            } else {
              val = 0;
            }
            put_byte(s, val);
          } while (val !== 0);
          if (s.gzhead.hcrc && s.pending > beg) {
            strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
          }
          if (val === 0) {
            s.gzindex = 0;
            s.status = COMMENT_STATE;
          }
        } else {
          s.status = COMMENT_STATE;
        }
      }
      if (s.status === COMMENT_STATE) {
        if (s.gzhead.comment) {
          beg = s.pending;
          do {
            if (s.pending === s.pending_buf_size) {
              if (s.gzhead.hcrc && s.pending > beg) {
                strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
              }
              flush_pending(strm);
              beg = s.pending;
              if (s.pending === s.pending_buf_size) {
                val = 1;
                break;
              }
            }
            if (s.gzindex < s.gzhead.comment.length) {
              val = s.gzhead.comment.charCodeAt(s.gzindex++) & 255;
            } else {
              val = 0;
            }
            put_byte(s, val);
          } while (val !== 0);
          if (s.gzhead.hcrc && s.pending > beg) {
            strm.adler = crc32(strm.adler, s.pending_buf, s.pending - beg, beg);
          }
          if (val === 0) {
            s.status = HCRC_STATE;
          }
        } else {
          s.status = HCRC_STATE;
        }
      }
      if (s.status === HCRC_STATE) {
        if (s.gzhead.hcrc) {
          if (s.pending + 2 > s.pending_buf_size) {
            flush_pending(strm);
          }
          if (s.pending + 2 <= s.pending_buf_size) {
            put_byte(s, strm.adler & 255);
            put_byte(s, strm.adler >> 8 & 255);
            strm.adler = 0;
            s.status = BUSY_STATE;
          }
        } else {
          s.status = BUSY_STATE;
        }
      }
      if (s.pending !== 0) {
        flush_pending(strm);
        if (strm.avail_out === 0) {
          s.last_flush = -1;
          return Z_OK;
        }
      } else if (strm.avail_in === 0 && rank(flush) <= rank(old_flush) && flush !== Z_FINISH) {
        return err(strm, Z_BUF_ERROR);
      }
      if (s.status === FINISH_STATE && strm.avail_in !== 0) {
        return err(strm, Z_BUF_ERROR);
      }
      if (strm.avail_in !== 0 || s.lookahead !== 0 || flush !== Z_NO_FLUSH && s.status !== FINISH_STATE) {
        var bstate = s.strategy === Z_HUFFMAN_ONLY ? deflate_huff(s, flush) : s.strategy === Z_RLE ? deflate_rle(s, flush) : configuration_table[s.level].func(s, flush);
        if (bstate === BS_FINISH_STARTED || bstate === BS_FINISH_DONE) {
          s.status = FINISH_STATE;
        }
        if (bstate === BS_NEED_MORE || bstate === BS_FINISH_STARTED) {
          if (strm.avail_out === 0) {
            s.last_flush = -1;
          }
          return Z_OK;
        }
        if (bstate === BS_BLOCK_DONE) {
          if (flush === Z_PARTIAL_FLUSH) {
            trees._tr_align(s);
          } else if (flush !== Z_BLOCK) {
            trees._tr_stored_block(s, 0, 0, false);
            if (flush === Z_FULL_FLUSH) {
              zero(s.head);
              if (s.lookahead === 0) {
                s.strstart = 0;
                s.block_start = 0;
                s.insert = 0;
              }
            }
          }
          flush_pending(strm);
          if (strm.avail_out === 0) {
            s.last_flush = -1;
            return Z_OK;
          }
        }
      }
      if (flush !== Z_FINISH) {
        return Z_OK;
      }
      if (s.wrap <= 0) {
        return Z_STREAM_END;
      }
      if (s.wrap === 2) {
        put_byte(s, strm.adler & 255);
        put_byte(s, strm.adler >> 8 & 255);
        put_byte(s, strm.adler >> 16 & 255);
        put_byte(s, strm.adler >> 24 & 255);
        put_byte(s, strm.total_in & 255);
        put_byte(s, strm.total_in >> 8 & 255);
        put_byte(s, strm.total_in >> 16 & 255);
        put_byte(s, strm.total_in >> 24 & 255);
      } else {
        putShortMSB(s, strm.adler >>> 16);
        putShortMSB(s, strm.adler & 65535);
      }
      flush_pending(strm);
      if (s.wrap > 0) {
        s.wrap = -s.wrap;
      }
      return s.pending !== 0 ? Z_OK : Z_STREAM_END;
    }
    function deflateEnd(strm) {
      var status;
      if (!strm || !strm.state) {
        return Z_STREAM_ERROR;
      }
      status = strm.state.status;
      if (status !== INIT_STATE && status !== EXTRA_STATE && status !== NAME_STATE && status !== COMMENT_STATE && status !== HCRC_STATE && status !== BUSY_STATE && status !== FINISH_STATE) {
        return err(strm, Z_STREAM_ERROR);
      }
      strm.state = null;
      return status === BUSY_STATE ? err(strm, Z_DATA_ERROR) : Z_OK;
    }
    function deflateSetDictionary(strm, dictionary) {
      var dictLength = dictionary.length;
      var s;
      var str, n;
      var wrap;
      var avail;
      var next;
      var input2;
      var tmpDict;
      if (!strm || !strm.state) {
        return Z_STREAM_ERROR;
      }
      s = strm.state;
      wrap = s.wrap;
      if (wrap === 2 || wrap === 1 && s.status !== INIT_STATE || s.lookahead) {
        return Z_STREAM_ERROR;
      }
      if (wrap === 1) {
        strm.adler = adler32(strm.adler, dictionary, dictLength, 0);
      }
      s.wrap = 0;
      if (dictLength >= s.w_size) {
        if (wrap === 0) {
          zero(s.head);
          s.strstart = 0;
          s.block_start = 0;
          s.insert = 0;
        }
        tmpDict = new utils.Buf8(s.w_size);
        utils.arraySet(tmpDict, dictionary, dictLength - s.w_size, s.w_size, 0);
        dictionary = tmpDict;
        dictLength = s.w_size;
      }
      avail = strm.avail_in;
      next = strm.next_in;
      input2 = strm.input;
      strm.avail_in = dictLength;
      strm.next_in = 0;
      strm.input = dictionary;
      fill_window(s);
      while (s.lookahead >= MIN_MATCH) {
        str = s.strstart;
        n = s.lookahead - (MIN_MATCH - 1);
        do {
          s.ins_h = (s.ins_h << s.hash_shift ^ s.window[str + MIN_MATCH - 1]) & s.hash_mask;
          s.prev[str & s.w_mask] = s.head[s.ins_h];
          s.head[s.ins_h] = str;
          str++;
        } while (--n);
        s.strstart = str;
        s.lookahead = MIN_MATCH - 1;
        fill_window(s);
      }
      s.strstart += s.lookahead;
      s.block_start = s.strstart;
      s.insert = s.lookahead;
      s.lookahead = 0;
      s.match_length = s.prev_length = MIN_MATCH - 1;
      s.match_available = 0;
      strm.next_in = next;
      strm.input = input2;
      strm.avail_in = avail;
      s.wrap = wrap;
      return Z_OK;
    }
    exports.deflateInit = deflateInit;
    exports.deflateInit2 = deflateInit2;
    exports.deflateReset = deflateReset;
    exports.deflateResetKeep = deflateResetKeep;
    exports.deflateSetHeader = deflateSetHeader;
    exports.deflate = deflate;
    exports.deflateEnd = deflateEnd;
    exports.deflateSetDictionary = deflateSetDictionary;
    exports.deflateInfo = "pako deflate (from Nodeca project)";
  }
});

// node_modules/pako/lib/utils/strings.js
var require_strings = __commonJS({
  "node_modules/pako/lib/utils/strings.js"(exports) {
    "use strict";
    var utils = require_common();
    var STR_APPLY_OK = true;
    var STR_APPLY_UIA_OK = true;
    try {
      String.fromCharCode.apply(null, [0]);
    } catch (__) {
      STR_APPLY_OK = false;
    }
    try {
      String.fromCharCode.apply(null, new Uint8Array(1));
    } catch (__) {
      STR_APPLY_UIA_OK = false;
    }
    var _utf8len = new utils.Buf8(256);
    for (q = 0; q < 256; q++) {
      _utf8len[q] = q >= 252 ? 6 : q >= 248 ? 5 : q >= 240 ? 4 : q >= 224 ? 3 : q >= 192 ? 2 : 1;
    }
    var q;
    _utf8len[254] = _utf8len[254] = 1;
    exports.string2buf = function(str) {
      var buf, c, c2, m_pos, i, str_len = str.length, buf_len = 0;
      for (m_pos = 0; m_pos < str_len; m_pos++) {
        c = str.charCodeAt(m_pos);
        if ((c & 64512) === 55296 && m_pos + 1 < str_len) {
          c2 = str.charCodeAt(m_pos + 1);
          if ((c2 & 64512) === 56320) {
            c = 65536 + (c - 55296 << 10) + (c2 - 56320);
            m_pos++;
          }
        }
        buf_len += c < 128 ? 1 : c < 2048 ? 2 : c < 65536 ? 3 : 4;
      }
      buf = new utils.Buf8(buf_len);
      for (i = 0, m_pos = 0; i < buf_len; m_pos++) {
        c = str.charCodeAt(m_pos);
        if ((c & 64512) === 55296 && m_pos + 1 < str_len) {
          c2 = str.charCodeAt(m_pos + 1);
          if ((c2 & 64512) === 56320) {
            c = 65536 + (c - 55296 << 10) + (c2 - 56320);
            m_pos++;
          }
        }
        if (c < 128) {
          buf[i++] = c;
        } else if (c < 2048) {
          buf[i++] = 192 | c >>> 6;
          buf[i++] = 128 | c & 63;
        } else if (c < 65536) {
          buf[i++] = 224 | c >>> 12;
          buf[i++] = 128 | c >>> 6 & 63;
          buf[i++] = 128 | c & 63;
        } else {
          buf[i++] = 240 | c >>> 18;
          buf[i++] = 128 | c >>> 12 & 63;
          buf[i++] = 128 | c >>> 6 & 63;
          buf[i++] = 128 | c & 63;
        }
      }
      return buf;
    };
    function buf2binstring(buf, len) {
      if (len < 65534) {
        if (buf.subarray && STR_APPLY_UIA_OK || !buf.subarray && STR_APPLY_OK) {
          return String.fromCharCode.apply(null, utils.shrinkBuf(buf, len));
        }
      }
      var result = "";
      for (var i = 0; i < len; i++) {
        result += String.fromCharCode(buf[i]);
      }
      return result;
    }
    exports.buf2binstring = function(buf) {
      return buf2binstring(buf, buf.length);
    };
    exports.binstring2buf = function(str) {
      var buf = new utils.Buf8(str.length);
      for (var i = 0, len = buf.length; i < len; i++) {
        buf[i] = str.charCodeAt(i);
      }
      return buf;
    };
    exports.buf2string = function(buf, max) {
      var i, out, c, c_len;
      var len = max || buf.length;
      var utf16buf = new Array(len * 2);
      for (out = 0, i = 0; i < len; ) {
        c = buf[i++];
        if (c < 128) {
          utf16buf[out++] = c;
          continue;
        }
        c_len = _utf8len[c];
        if (c_len > 4) {
          utf16buf[out++] = 65533;
          i += c_len - 1;
          continue;
        }
        c &= c_len === 2 ? 31 : c_len === 3 ? 15 : 7;
        while (c_len > 1 && i < len) {
          c = c << 6 | buf[i++] & 63;
          c_len--;
        }
        if (c_len > 1) {
          utf16buf[out++] = 65533;
          continue;
        }
        if (c < 65536) {
          utf16buf[out++] = c;
        } else {
          c -= 65536;
          utf16buf[out++] = 55296 | c >> 10 & 1023;
          utf16buf[out++] = 56320 | c & 1023;
        }
      }
      return buf2binstring(utf16buf, out);
    };
    exports.utf8border = function(buf, max) {
      var pos;
      max = max || buf.length;
      if (max > buf.length) {
        max = buf.length;
      }
      pos = max - 1;
      while (pos >= 0 && (buf[pos] & 192) === 128) {
        pos--;
      }
      if (pos < 0) {
        return max;
      }
      if (pos === 0) {
        return max;
      }
      return pos + _utf8len[buf[pos]] > max ? pos : max;
    };
  }
});

// node_modules/pako/lib/zlib/zstream.js
var require_zstream = __commonJS({
  "node_modules/pako/lib/zlib/zstream.js"(exports, module) {
    "use strict";
    function ZStream() {
      this.input = null;
      this.next_in = 0;
      this.avail_in = 0;
      this.total_in = 0;
      this.output = null;
      this.next_out = 0;
      this.avail_out = 0;
      this.total_out = 0;
      this.msg = "";
      this.state = null;
      this.data_type = 2;
      this.adler = 0;
    }
    module.exports = ZStream;
  }
});

// node_modules/pako/lib/deflate.js
var require_deflate2 = __commonJS({
  "node_modules/pako/lib/deflate.js"(exports) {
    "use strict";
    var zlib_deflate = require_deflate();
    var utils = require_common();
    var strings = require_strings();
    var msg = require_messages();
    var ZStream = require_zstream();
    var toString = Object.prototype.toString;
    var Z_NO_FLUSH = 0;
    var Z_FINISH = 4;
    var Z_OK = 0;
    var Z_STREAM_END = 1;
    var Z_SYNC_FLUSH = 2;
    var Z_DEFAULT_COMPRESSION = -1;
    var Z_DEFAULT_STRATEGY = 0;
    var Z_DEFLATED = 8;
    function Deflate(options) {
      if (!(this instanceof Deflate)) return new Deflate(options);
      this.options = utils.assign({
        level: Z_DEFAULT_COMPRESSION,
        method: Z_DEFLATED,
        chunkSize: 16384,
        windowBits: 15,
        memLevel: 8,
        strategy: Z_DEFAULT_STRATEGY,
        to: ""
      }, options || {});
      var opt = this.options;
      if (opt.raw && opt.windowBits > 0) {
        opt.windowBits = -opt.windowBits;
      } else if (opt.gzip && opt.windowBits > 0 && opt.windowBits < 16) {
        opt.windowBits += 16;
      }
      this.err = 0;
      this.msg = "";
      this.ended = false;
      this.chunks = [];
      this.strm = new ZStream();
      this.strm.avail_out = 0;
      var status = zlib_deflate.deflateInit2(
        this.strm,
        opt.level,
        opt.method,
        opt.windowBits,
        opt.memLevel,
        opt.strategy
      );
      if (status !== Z_OK) {
        throw new Error(msg[status]);
      }
      if (opt.header) {
        zlib_deflate.deflateSetHeader(this.strm, opt.header);
      }
      if (opt.dictionary) {
        var dict;
        if (typeof opt.dictionary === "string") {
          dict = strings.string2buf(opt.dictionary);
        } else if (toString.call(opt.dictionary) === "[object ArrayBuffer]") {
          dict = new Uint8Array(opt.dictionary);
        } else {
          dict = opt.dictionary;
        }
        status = zlib_deflate.deflateSetDictionary(this.strm, dict);
        if (status !== Z_OK) {
          throw new Error(msg[status]);
        }
        this._dict_set = true;
      }
    }
    Deflate.prototype.push = function(data, mode) {
      var strm = this.strm;
      var chunkSize = this.options.chunkSize;
      var status, _mode;
      if (this.ended) {
        return false;
      }
      _mode = mode === ~~mode ? mode : mode === true ? Z_FINISH : Z_NO_FLUSH;
      if (typeof data === "string") {
        strm.input = strings.string2buf(data);
      } else if (toString.call(data) === "[object ArrayBuffer]") {
        strm.input = new Uint8Array(data);
      } else {
        strm.input = data;
      }
      strm.next_in = 0;
      strm.avail_in = strm.input.length;
      do {
        if (strm.avail_out === 0) {
          strm.output = new utils.Buf8(chunkSize);
          strm.next_out = 0;
          strm.avail_out = chunkSize;
        }
        status = zlib_deflate.deflate(strm, _mode);
        if (status !== Z_STREAM_END && status !== Z_OK) {
          this.onEnd(status);
          this.ended = true;
          return false;
        }
        if (strm.avail_out === 0 || strm.avail_in === 0 && (_mode === Z_FINISH || _mode === Z_SYNC_FLUSH)) {
          if (this.options.to === "string") {
            this.onData(strings.buf2binstring(utils.shrinkBuf(strm.output, strm.next_out)));
          } else {
            this.onData(utils.shrinkBuf(strm.output, strm.next_out));
          }
        }
      } while ((strm.avail_in > 0 || strm.avail_out === 0) && status !== Z_STREAM_END);
      if (_mode === Z_FINISH) {
        status = zlib_deflate.deflateEnd(this.strm);
        this.onEnd(status);
        this.ended = true;
        return status === Z_OK;
      }
      if (_mode === Z_SYNC_FLUSH) {
        this.onEnd(Z_OK);
        strm.avail_out = 0;
        return true;
      }
      return true;
    };
    Deflate.prototype.onData = function(chunk) {
      this.chunks.push(chunk);
    };
    Deflate.prototype.onEnd = function(status) {
      if (status === Z_OK) {
        if (this.options.to === "string") {
          this.result = this.chunks.join("");
        } else {
          this.result = utils.flattenChunks(this.chunks);
        }
      }
      this.chunks = [];
      this.err = status;
      this.msg = this.strm.msg;
    };
    function deflate(input2, options) {
      var deflator = new Deflate(options);
      deflator.push(input2, true);
      if (deflator.err) {
        throw deflator.msg || msg[deflator.err];
      }
      return deflator.result;
    }
    function deflateRaw(input2, options) {
      options = options || {};
      options.raw = true;
      return deflate(input2, options);
    }
    function gzip(input2, options) {
      options = options || {};
      options.gzip = true;
      return deflate(input2, options);
    }
    exports.Deflate = Deflate;
    exports.deflate = deflate;
    exports.deflateRaw = deflateRaw;
    exports.gzip = gzip;
  }
});

// node_modules/pako/lib/zlib/inffast.js
var require_inffast = __commonJS({
  "node_modules/pako/lib/zlib/inffast.js"(exports, module) {
    "use strict";
    var BAD = 30;
    var TYPE = 12;
    module.exports = function inflate_fast(strm, start) {
      var state;
      var _in;
      var last;
      var _out;
      var beg;
      var end;
      var dmax;
      var wsize;
      var whave;
      var wnext;
      var s_window;
      var hold;
      var bits;
      var lcode;
      var dcode;
      var lmask;
      var dmask;
      var here;
      var op;
      var len;
      var dist;
      var from;
      var from_source;
      var input2, output2;
      state = strm.state;
      _in = strm.next_in;
      input2 = strm.input;
      last = _in + (strm.avail_in - 5);
      _out = strm.next_out;
      output2 = strm.output;
      beg = _out - (start - strm.avail_out);
      end = _out + (strm.avail_out - 257);
      dmax = state.dmax;
      wsize = state.wsize;
      whave = state.whave;
      wnext = state.wnext;
      s_window = state.window;
      hold = state.hold;
      bits = state.bits;
      lcode = state.lencode;
      dcode = state.distcode;
      lmask = (1 << state.lenbits) - 1;
      dmask = (1 << state.distbits) - 1;
      top:
        do {
          if (bits < 15) {
            hold += input2[_in++] << bits;
            bits += 8;
            hold += input2[_in++] << bits;
            bits += 8;
          }
          here = lcode[hold & lmask];
          dolen:
            for (; ; ) {
              op = here >>> 24;
              hold >>>= op;
              bits -= op;
              op = here >>> 16 & 255;
              if (op === 0) {
                output2[_out++] = here & 65535;
              } else if (op & 16) {
                len = here & 65535;
                op &= 15;
                if (op) {
                  if (bits < op) {
                    hold += input2[_in++] << bits;
                    bits += 8;
                  }
                  len += hold & (1 << op) - 1;
                  hold >>>= op;
                  bits -= op;
                }
                if (bits < 15) {
                  hold += input2[_in++] << bits;
                  bits += 8;
                  hold += input2[_in++] << bits;
                  bits += 8;
                }
                here = dcode[hold & dmask];
                dodist:
                  for (; ; ) {
                    op = here >>> 24;
                    hold >>>= op;
                    bits -= op;
                    op = here >>> 16 & 255;
                    if (op & 16) {
                      dist = here & 65535;
                      op &= 15;
                      if (bits < op) {
                        hold += input2[_in++] << bits;
                        bits += 8;
                        if (bits < op) {
                          hold += input2[_in++] << bits;
                          bits += 8;
                        }
                      }
                      dist += hold & (1 << op) - 1;
                      if (dist > dmax) {
                        strm.msg = "invalid distance too far back";
                        state.mode = BAD;
                        break top;
                      }
                      hold >>>= op;
                      bits -= op;
                      op = _out - beg;
                      if (dist > op) {
                        op = dist - op;
                        if (op > whave) {
                          if (state.sane) {
                            strm.msg = "invalid distance too far back";
                            state.mode = BAD;
                            break top;
                          }
                        }
                        from = 0;
                        from_source = s_window;
                        if (wnext === 0) {
                          from += wsize - op;
                          if (op < len) {
                            len -= op;
                            do {
                              output2[_out++] = s_window[from++];
                            } while (--op);
                            from = _out - dist;
                            from_source = output2;
                          }
                        } else if (wnext < op) {
                          from += wsize + wnext - op;
                          op -= wnext;
                          if (op < len) {
                            len -= op;
                            do {
                              output2[_out++] = s_window[from++];
                            } while (--op);
                            from = 0;
                            if (wnext < len) {
                              op = wnext;
                              len -= op;
                              do {
                                output2[_out++] = s_window[from++];
                              } while (--op);
                              from = _out - dist;
                              from_source = output2;
                            }
                          }
                        } else {
                          from += wnext - op;
                          if (op < len) {
                            len -= op;
                            do {
                              output2[_out++] = s_window[from++];
                            } while (--op);
                            from = _out - dist;
                            from_source = output2;
                          }
                        }
                        while (len > 2) {
                          output2[_out++] = from_source[from++];
                          output2[_out++] = from_source[from++];
                          output2[_out++] = from_source[from++];
                          len -= 3;
                        }
                        if (len) {
                          output2[_out++] = from_source[from++];
                          if (len > 1) {
                            output2[_out++] = from_source[from++];
                          }
                        }
                      } else {
                        from = _out - dist;
                        do {
                          output2[_out++] = output2[from++];
                          output2[_out++] = output2[from++];
                          output2[_out++] = output2[from++];
                          len -= 3;
                        } while (len > 2);
                        if (len) {
                          output2[_out++] = output2[from++];
                          if (len > 1) {
                            output2[_out++] = output2[from++];
                          }
                        }
                      }
                    } else if ((op & 64) === 0) {
                      here = dcode[(here & 65535) + (hold & (1 << op) - 1)];
                      continue dodist;
                    } else {
                      strm.msg = "invalid distance code";
                      state.mode = BAD;
                      break top;
                    }
                    break;
                  }
              } else if ((op & 64) === 0) {
                here = lcode[(here & 65535) + (hold & (1 << op) - 1)];
                continue dolen;
              } else if (op & 32) {
                state.mode = TYPE;
                break top;
              } else {
                strm.msg = "invalid literal/length code";
                state.mode = BAD;
                break top;
              }
              break;
            }
        } while (_in < last && _out < end);
      len = bits >> 3;
      _in -= len;
      bits -= len << 3;
      hold &= (1 << bits) - 1;
      strm.next_in = _in;
      strm.next_out = _out;
      strm.avail_in = _in < last ? 5 + (last - _in) : 5 - (_in - last);
      strm.avail_out = _out < end ? 257 + (end - _out) : 257 - (_out - end);
      state.hold = hold;
      state.bits = bits;
      return;
    };
  }
});

// node_modules/pako/lib/zlib/inftrees.js
var require_inftrees = __commonJS({
  "node_modules/pako/lib/zlib/inftrees.js"(exports, module) {
    "use strict";
    var utils = require_common();
    var MAXBITS = 15;
    var ENOUGH_LENS = 852;
    var ENOUGH_DISTS = 592;
    var CODES = 0;
    var LENS = 1;
    var DISTS = 2;
    var lbase = [
      /* Length codes 257..285 base */
      3,
      4,
      5,
      6,
      7,
      8,
      9,
      10,
      11,
      13,
      15,
      17,
      19,
      23,
      27,
      31,
      35,
      43,
      51,
      59,
      67,
      83,
      99,
      115,
      131,
      163,
      195,
      227,
      258,
      0,
      0
    ];
    var lext = [
      /* Length codes 257..285 extra */
      16,
      16,
      16,
      16,
      16,
      16,
      16,
      16,
      17,
      17,
      17,
      17,
      18,
      18,
      18,
      18,
      19,
      19,
      19,
      19,
      20,
      20,
      20,
      20,
      21,
      21,
      21,
      21,
      16,
      72,
      78
    ];
    var dbase = [
      /* Distance codes 0..29 base */
      1,
      2,
      3,
      4,
      5,
      7,
      9,
      13,
      17,
      25,
      33,
      49,
      65,
      97,
      129,
      193,
      257,
      385,
      513,
      769,
      1025,
      1537,
      2049,
      3073,
      4097,
      6145,
      8193,
      12289,
      16385,
      24577,
      0,
      0
    ];
    var dext = [
      /* Distance codes 0..29 extra */
      16,
      16,
      16,
      16,
      17,
      17,
      18,
      18,
      19,
      19,
      20,
      20,
      21,
      21,
      22,
      22,
      23,
      23,
      24,
      24,
      25,
      25,
      26,
      26,
      27,
      27,
      28,
      28,
      29,
      29,
      64,
      64
    ];
    module.exports = function inflate_table(type, lens, lens_index, codes, table, table_index, work, opts) {
      var bits = opts.bits;
      var len = 0;
      var sym = 0;
      var min = 0, max = 0;
      var root = 0;
      var curr = 0;
      var drop = 0;
      var left = 0;
      var used = 0;
      var huff = 0;
      var incr;
      var fill;
      var low;
      var mask;
      var next;
      var base = null;
      var base_index = 0;
      var end;
      var count = new utils.Buf16(MAXBITS + 1);
      var offs = new utils.Buf16(MAXBITS + 1);
      var extra = null;
      var extra_index = 0;
      var here_bits, here_op, here_val;
      for (len = 0; len <= MAXBITS; len++) {
        count[len] = 0;
      }
      for (sym = 0; sym < codes; sym++) {
        count[lens[lens_index + sym]]++;
      }
      root = bits;
      for (max = MAXBITS; max >= 1; max--) {
        if (count[max] !== 0) {
          break;
        }
      }
      if (root > max) {
        root = max;
      }
      if (max === 0) {
        table[table_index++] = 1 << 24 | 64 << 16 | 0;
        table[table_index++] = 1 << 24 | 64 << 16 | 0;
        opts.bits = 1;
        return 0;
      }
      for (min = 1; min < max; min++) {
        if (count[min] !== 0) {
          break;
        }
      }
      if (root < min) {
        root = min;
      }
      left = 1;
      for (len = 1; len <= MAXBITS; len++) {
        left <<= 1;
        left -= count[len];
        if (left < 0) {
          return -1;
        }
      }
      if (left > 0 && (type === CODES || max !== 1)) {
        return -1;
      }
      offs[1] = 0;
      for (len = 1; len < MAXBITS; len++) {
        offs[len + 1] = offs[len] + count[len];
      }
      for (sym = 0; sym < codes; sym++) {
        if (lens[lens_index + sym] !== 0) {
          work[offs[lens[lens_index + sym]]++] = sym;
        }
      }
      if (type === CODES) {
        base = extra = work;
        end = 19;
      } else if (type === LENS) {
        base = lbase;
        base_index -= 257;
        extra = lext;
        extra_index -= 257;
        end = 256;
      } else {
        base = dbase;
        extra = dext;
        end = -1;
      }
      huff = 0;
      sym = 0;
      len = min;
      next = table_index;
      curr = root;
      drop = 0;
      low = -1;
      used = 1 << root;
      mask = used - 1;
      if (type === LENS && used > ENOUGH_LENS || type === DISTS && used > ENOUGH_DISTS) {
        return 1;
      }
      for (; ; ) {
        here_bits = len - drop;
        if (work[sym] < end) {
          here_op = 0;
          here_val = work[sym];
        } else if (work[sym] > end) {
          here_op = extra[extra_index + work[sym]];
          here_val = base[base_index + work[sym]];
        } else {
          here_op = 32 + 64;
          here_val = 0;
        }
        incr = 1 << len - drop;
        fill = 1 << curr;
        min = fill;
        do {
          fill -= incr;
          table[next + (huff >> drop) + fill] = here_bits << 24 | here_op << 16 | here_val | 0;
        } while (fill !== 0);
        incr = 1 << len - 1;
        while (huff & incr) {
          incr >>= 1;
        }
        if (incr !== 0) {
          huff &= incr - 1;
          huff += incr;
        } else {
          huff = 0;
        }
        sym++;
        if (--count[len] === 0) {
          if (len === max) {
            break;
          }
          len = lens[lens_index + work[sym]];
        }
        if (len > root && (huff & mask) !== low) {
          if (drop === 0) {
            drop = root;
          }
          next += min;
          curr = len - drop;
          left = 1 << curr;
          while (curr + drop < max) {
            left -= count[curr + drop];
            if (left <= 0) {
              break;
            }
            curr++;
            left <<= 1;
          }
          used += 1 << curr;
          if (type === LENS && used > ENOUGH_LENS || type === DISTS && used > ENOUGH_DISTS) {
            return 1;
          }
          low = huff & mask;
          table[low] = root << 24 | curr << 16 | next - table_index | 0;
        }
      }
      if (huff !== 0) {
        table[next + huff] = len - drop << 24 | 64 << 16 | 0;
      }
      opts.bits = root;
      return 0;
    };
  }
});

// node_modules/pako/lib/zlib/inflate.js
var require_inflate = __commonJS({
  "node_modules/pako/lib/zlib/inflate.js"(exports) {
    "use strict";
    var utils = require_common();
    var adler32 = require_adler32();
    var crc32 = require_crc322();
    var inflate_fast = require_inffast();
    var inflate_table = require_inftrees();
    var CODES = 0;
    var LENS = 1;
    var DISTS = 2;
    var Z_FINISH = 4;
    var Z_BLOCK = 5;
    var Z_TREES = 6;
    var Z_OK = 0;
    var Z_STREAM_END = 1;
    var Z_NEED_DICT = 2;
    var Z_STREAM_ERROR = -2;
    var Z_DATA_ERROR = -3;
    var Z_MEM_ERROR = -4;
    var Z_BUF_ERROR = -5;
    var Z_DEFLATED = 8;
    var HEAD = 1;
    var FLAGS = 2;
    var TIME = 3;
    var OS = 4;
    var EXLEN = 5;
    var EXTRA = 6;
    var NAME = 7;
    var COMMENT = 8;
    var HCRC = 9;
    var DICTID = 10;
    var DICT = 11;
    var TYPE = 12;
    var TYPEDO = 13;
    var STORED = 14;
    var COPY_ = 15;
    var COPY = 16;
    var TABLE = 17;
    var LENLENS = 18;
    var CODELENS = 19;
    var LEN_ = 20;
    var LEN = 21;
    var LENEXT = 22;
    var DIST = 23;
    var DISTEXT = 24;
    var MATCH = 25;
    var LIT = 26;
    var CHECK = 27;
    var LENGTH = 28;
    var DONE = 29;
    var BAD = 30;
    var MEM = 31;
    var SYNC = 32;
    var ENOUGH_LENS = 852;
    var ENOUGH_DISTS = 592;
    var MAX_WBITS = 15;
    var DEF_WBITS = MAX_WBITS;
    function zswap32(q) {
      return (q >>> 24 & 255) + (q >>> 8 & 65280) + ((q & 65280) << 8) + ((q & 255) << 24);
    }
    function InflateState() {
      this.mode = 0;
      this.last = false;
      this.wrap = 0;
      this.havedict = false;
      this.flags = 0;
      this.dmax = 0;
      this.check = 0;
      this.total = 0;
      this.head = null;
      this.wbits = 0;
      this.wsize = 0;
      this.whave = 0;
      this.wnext = 0;
      this.window = null;
      this.hold = 0;
      this.bits = 0;
      this.length = 0;
      this.offset = 0;
      this.extra = 0;
      this.lencode = null;
      this.distcode = null;
      this.lenbits = 0;
      this.distbits = 0;
      this.ncode = 0;
      this.nlen = 0;
      this.ndist = 0;
      this.have = 0;
      this.next = null;
      this.lens = new utils.Buf16(320);
      this.work = new utils.Buf16(288);
      this.lendyn = null;
      this.distdyn = null;
      this.sane = 0;
      this.back = 0;
      this.was = 0;
    }
    function inflateResetKeep(strm) {
      var state;
      if (!strm || !strm.state) {
        return Z_STREAM_ERROR;
      }
      state = strm.state;
      strm.total_in = strm.total_out = state.total = 0;
      strm.msg = "";
      if (state.wrap) {
        strm.adler = state.wrap & 1;
      }
      state.mode = HEAD;
      state.last = 0;
      state.havedict = 0;
      state.dmax = 32768;
      state.head = null;
      state.hold = 0;
      state.bits = 0;
      state.lencode = state.lendyn = new utils.Buf32(ENOUGH_LENS);
      state.distcode = state.distdyn = new utils.Buf32(ENOUGH_DISTS);
      state.sane = 1;
      state.back = -1;
      return Z_OK;
    }
    function inflateReset(strm) {
      var state;
      if (!strm || !strm.state) {
        return Z_STREAM_ERROR;
      }
      state = strm.state;
      state.wsize = 0;
      state.whave = 0;
      state.wnext = 0;
      return inflateResetKeep(strm);
    }
    function inflateReset2(strm, windowBits) {
      var wrap;
      var state;
      if (!strm || !strm.state) {
        return Z_STREAM_ERROR;
      }
      state = strm.state;
      if (windowBits < 0) {
        wrap = 0;
        windowBits = -windowBits;
      } else {
        wrap = (windowBits >> 4) + 1;
        if (windowBits < 48) {
          windowBits &= 15;
        }
      }
      if (windowBits && (windowBits < 8 || windowBits > 15)) {
        return Z_STREAM_ERROR;
      }
      if (state.window !== null && state.wbits !== windowBits) {
        state.window = null;
      }
      state.wrap = wrap;
      state.wbits = windowBits;
      return inflateReset(strm);
    }
    function inflateInit2(strm, windowBits) {
      var ret;
      var state;
      if (!strm) {
        return Z_STREAM_ERROR;
      }
      state = new InflateState();
      strm.state = state;
      state.window = null;
      ret = inflateReset2(strm, windowBits);
      if (ret !== Z_OK) {
        strm.state = null;
      }
      return ret;
    }
    function inflateInit(strm) {
      return inflateInit2(strm, DEF_WBITS);
    }
    var virgin = true;
    var lenfix;
    var distfix;
    function fixedtables(state) {
      if (virgin) {
        var sym;
        lenfix = new utils.Buf32(512);
        distfix = new utils.Buf32(32);
        sym = 0;
        while (sym < 144) {
          state.lens[sym++] = 8;
        }
        while (sym < 256) {
          state.lens[sym++] = 9;
        }
        while (sym < 280) {
          state.lens[sym++] = 7;
        }
        while (sym < 288) {
          state.lens[sym++] = 8;
        }
        inflate_table(LENS, state.lens, 0, 288, lenfix, 0, state.work, { bits: 9 });
        sym = 0;
        while (sym < 32) {
          state.lens[sym++] = 5;
        }
        inflate_table(DISTS, state.lens, 0, 32, distfix, 0, state.work, { bits: 5 });
        virgin = false;
      }
      state.lencode = lenfix;
      state.lenbits = 9;
      state.distcode = distfix;
      state.distbits = 5;
    }
    function updatewindow(strm, src, end, copy) {
      var dist;
      var state = strm.state;
      if (state.window === null) {
        state.wsize = 1 << state.wbits;
        state.wnext = 0;
        state.whave = 0;
        state.window = new utils.Buf8(state.wsize);
      }
      if (copy >= state.wsize) {
        utils.arraySet(state.window, src, end - state.wsize, state.wsize, 0);
        state.wnext = 0;
        state.whave = state.wsize;
      } else {
        dist = state.wsize - state.wnext;
        if (dist > copy) {
          dist = copy;
        }
        utils.arraySet(state.window, src, end - copy, dist, state.wnext);
        copy -= dist;
        if (copy) {
          utils.arraySet(state.window, src, end - copy, copy, 0);
          state.wnext = copy;
          state.whave = state.wsize;
        } else {
          state.wnext += dist;
          if (state.wnext === state.wsize) {
            state.wnext = 0;
          }
          if (state.whave < state.wsize) {
            state.whave += dist;
          }
        }
      }
      return 0;
    }
    function inflate(strm, flush) {
      var state;
      var input2, output2;
      var next;
      var put;
      var have, left;
      var hold;
      var bits;
      var _in, _out;
      var copy;
      var from;
      var from_source;
      var here = 0;
      var here_bits, here_op, here_val;
      var last_bits, last_op, last_val;
      var len;
      var ret;
      var hbuf = new utils.Buf8(4);
      var opts;
      var n;
      var order = (
        /* permutation of code lengths */
        [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15]
      );
      if (!strm || !strm.state || !strm.output || !strm.input && strm.avail_in !== 0) {
        return Z_STREAM_ERROR;
      }
      state = strm.state;
      if (state.mode === TYPE) {
        state.mode = TYPEDO;
      }
      put = strm.next_out;
      output2 = strm.output;
      left = strm.avail_out;
      next = strm.next_in;
      input2 = strm.input;
      have = strm.avail_in;
      hold = state.hold;
      bits = state.bits;
      _in = have;
      _out = left;
      ret = Z_OK;
      inf_leave:
        for (; ; ) {
          switch (state.mode) {
            case HEAD:
              if (state.wrap === 0) {
                state.mode = TYPEDO;
                break;
              }
              while (bits < 16) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              if (state.wrap & 2 && hold === 35615) {
                state.check = 0;
                hbuf[0] = hold & 255;
                hbuf[1] = hold >>> 8 & 255;
                state.check = crc32(state.check, hbuf, 2, 0);
                hold = 0;
                bits = 0;
                state.mode = FLAGS;
                break;
              }
              state.flags = 0;
              if (state.head) {
                state.head.done = false;
              }
              if (!(state.wrap & 1) || /* check if zlib header allowed */
              (((hold & 255) << 8) + (hold >> 8)) % 31) {
                strm.msg = "incorrect header check";
                state.mode = BAD;
                break;
              }
              if ((hold & 15) !== Z_DEFLATED) {
                strm.msg = "unknown compression method";
                state.mode = BAD;
                break;
              }
              hold >>>= 4;
              bits -= 4;
              len = (hold & 15) + 8;
              if (state.wbits === 0) {
                state.wbits = len;
              } else if (len > state.wbits) {
                strm.msg = "invalid window size";
                state.mode = BAD;
                break;
              }
              state.dmax = 1 << len;
              strm.adler = state.check = 1;
              state.mode = hold & 512 ? DICTID : TYPE;
              hold = 0;
              bits = 0;
              break;
            case FLAGS:
              while (bits < 16) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              state.flags = hold;
              if ((state.flags & 255) !== Z_DEFLATED) {
                strm.msg = "unknown compression method";
                state.mode = BAD;
                break;
              }
              if (state.flags & 57344) {
                strm.msg = "unknown header flags set";
                state.mode = BAD;
                break;
              }
              if (state.head) {
                state.head.text = hold >> 8 & 1;
              }
              if (state.flags & 512) {
                hbuf[0] = hold & 255;
                hbuf[1] = hold >>> 8 & 255;
                state.check = crc32(state.check, hbuf, 2, 0);
              }
              hold = 0;
              bits = 0;
              state.mode = TIME;
            /* falls through */
            case TIME:
              while (bits < 32) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              if (state.head) {
                state.head.time = hold;
              }
              if (state.flags & 512) {
                hbuf[0] = hold & 255;
                hbuf[1] = hold >>> 8 & 255;
                hbuf[2] = hold >>> 16 & 255;
                hbuf[3] = hold >>> 24 & 255;
                state.check = crc32(state.check, hbuf, 4, 0);
              }
              hold = 0;
              bits = 0;
              state.mode = OS;
            /* falls through */
            case OS:
              while (bits < 16) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              if (state.head) {
                state.head.xflags = hold & 255;
                state.head.os = hold >> 8;
              }
              if (state.flags & 512) {
                hbuf[0] = hold & 255;
                hbuf[1] = hold >>> 8 & 255;
                state.check = crc32(state.check, hbuf, 2, 0);
              }
              hold = 0;
              bits = 0;
              state.mode = EXLEN;
            /* falls through */
            case EXLEN:
              if (state.flags & 1024) {
                while (bits < 16) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input2[next++] << bits;
                  bits += 8;
                }
                state.length = hold;
                if (state.head) {
                  state.head.extra_len = hold;
                }
                if (state.flags & 512) {
                  hbuf[0] = hold & 255;
                  hbuf[1] = hold >>> 8 & 255;
                  state.check = crc32(state.check, hbuf, 2, 0);
                }
                hold = 0;
                bits = 0;
              } else if (state.head) {
                state.head.extra = null;
              }
              state.mode = EXTRA;
            /* falls through */
            case EXTRA:
              if (state.flags & 1024) {
                copy = state.length;
                if (copy > have) {
                  copy = have;
                }
                if (copy) {
                  if (state.head) {
                    len = state.head.extra_len - state.length;
                    if (!state.head.extra) {
                      state.head.extra = new Array(state.head.extra_len);
                    }
                    utils.arraySet(
                      state.head.extra,
                      input2,
                      next,
                      // extra field is limited to 65536 bytes
                      // - no need for additional size check
                      copy,
                      /*len + copy > state.head.extra_max - len ? state.head.extra_max : copy,*/
                      len
                    );
                  }
                  if (state.flags & 512) {
                    state.check = crc32(state.check, input2, copy, next);
                  }
                  have -= copy;
                  next += copy;
                  state.length -= copy;
                }
                if (state.length) {
                  break inf_leave;
                }
              }
              state.length = 0;
              state.mode = NAME;
            /* falls through */
            case NAME:
              if (state.flags & 2048) {
                if (have === 0) {
                  break inf_leave;
                }
                copy = 0;
                do {
                  len = input2[next + copy++];
                  if (state.head && len && state.length < 65536) {
                    state.head.name += String.fromCharCode(len);
                  }
                } while (len && copy < have);
                if (state.flags & 512) {
                  state.check = crc32(state.check, input2, copy, next);
                }
                have -= copy;
                next += copy;
                if (len) {
                  break inf_leave;
                }
              } else if (state.head) {
                state.head.name = null;
              }
              state.length = 0;
              state.mode = COMMENT;
            /* falls through */
            case COMMENT:
              if (state.flags & 4096) {
                if (have === 0) {
                  break inf_leave;
                }
                copy = 0;
                do {
                  len = input2[next + copy++];
                  if (state.head && len && state.length < 65536) {
                    state.head.comment += String.fromCharCode(len);
                  }
                } while (len && copy < have);
                if (state.flags & 512) {
                  state.check = crc32(state.check, input2, copy, next);
                }
                have -= copy;
                next += copy;
                if (len) {
                  break inf_leave;
                }
              } else if (state.head) {
                state.head.comment = null;
              }
              state.mode = HCRC;
            /* falls through */
            case HCRC:
              if (state.flags & 512) {
                while (bits < 16) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input2[next++] << bits;
                  bits += 8;
                }
                if (hold !== (state.check & 65535)) {
                  strm.msg = "header crc mismatch";
                  state.mode = BAD;
                  break;
                }
                hold = 0;
                bits = 0;
              }
              if (state.head) {
                state.head.hcrc = state.flags >> 9 & 1;
                state.head.done = true;
              }
              strm.adler = state.check = 0;
              state.mode = TYPE;
              break;
            case DICTID:
              while (bits < 32) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              strm.adler = state.check = zswap32(hold);
              hold = 0;
              bits = 0;
              state.mode = DICT;
            /* falls through */
            case DICT:
              if (state.havedict === 0) {
                strm.next_out = put;
                strm.avail_out = left;
                strm.next_in = next;
                strm.avail_in = have;
                state.hold = hold;
                state.bits = bits;
                return Z_NEED_DICT;
              }
              strm.adler = state.check = 1;
              state.mode = TYPE;
            /* falls through */
            case TYPE:
              if (flush === Z_BLOCK || flush === Z_TREES) {
                break inf_leave;
              }
            /* falls through */
            case TYPEDO:
              if (state.last) {
                hold >>>= bits & 7;
                bits -= bits & 7;
                state.mode = CHECK;
                break;
              }
              while (bits < 3) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              state.last = hold & 1;
              hold >>>= 1;
              bits -= 1;
              switch (hold & 3) {
                case 0:
                  state.mode = STORED;
                  break;
                case 1:
                  fixedtables(state);
                  state.mode = LEN_;
                  if (flush === Z_TREES) {
                    hold >>>= 2;
                    bits -= 2;
                    break inf_leave;
                  }
                  break;
                case 2:
                  state.mode = TABLE;
                  break;
                case 3:
                  strm.msg = "invalid block type";
                  state.mode = BAD;
              }
              hold >>>= 2;
              bits -= 2;
              break;
            case STORED:
              hold >>>= bits & 7;
              bits -= bits & 7;
              while (bits < 32) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              if ((hold & 65535) !== (hold >>> 16 ^ 65535)) {
                strm.msg = "invalid stored block lengths";
                state.mode = BAD;
                break;
              }
              state.length = hold & 65535;
              hold = 0;
              bits = 0;
              state.mode = COPY_;
              if (flush === Z_TREES) {
                break inf_leave;
              }
            /* falls through */
            case COPY_:
              state.mode = COPY;
            /* falls through */
            case COPY:
              copy = state.length;
              if (copy) {
                if (copy > have) {
                  copy = have;
                }
                if (copy > left) {
                  copy = left;
                }
                if (copy === 0) {
                  break inf_leave;
                }
                utils.arraySet(output2, input2, next, copy, put);
                have -= copy;
                next += copy;
                left -= copy;
                put += copy;
                state.length -= copy;
                break;
              }
              state.mode = TYPE;
              break;
            case TABLE:
              while (bits < 14) {
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              state.nlen = (hold & 31) + 257;
              hold >>>= 5;
              bits -= 5;
              state.ndist = (hold & 31) + 1;
              hold >>>= 5;
              bits -= 5;
              state.ncode = (hold & 15) + 4;
              hold >>>= 4;
              bits -= 4;
              if (state.nlen > 286 || state.ndist > 30) {
                strm.msg = "too many length or distance symbols";
                state.mode = BAD;
                break;
              }
              state.have = 0;
              state.mode = LENLENS;
            /* falls through */
            case LENLENS:
              while (state.have < state.ncode) {
                while (bits < 3) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input2[next++] << bits;
                  bits += 8;
                }
                state.lens[order[state.have++]] = hold & 7;
                hold >>>= 3;
                bits -= 3;
              }
              while (state.have < 19) {
                state.lens[order[state.have++]] = 0;
              }
              state.lencode = state.lendyn;
              state.lenbits = 7;
              opts = { bits: state.lenbits };
              ret = inflate_table(CODES, state.lens, 0, 19, state.lencode, 0, state.work, opts);
              state.lenbits = opts.bits;
              if (ret) {
                strm.msg = "invalid code lengths set";
                state.mode = BAD;
                break;
              }
              state.have = 0;
              state.mode = CODELENS;
            /* falls through */
            case CODELENS:
              while (state.have < state.nlen + state.ndist) {
                for (; ; ) {
                  here = state.lencode[hold & (1 << state.lenbits) - 1];
                  here_bits = here >>> 24;
                  here_op = here >>> 16 & 255;
                  here_val = here & 65535;
                  if (here_bits <= bits) {
                    break;
                  }
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input2[next++] << bits;
                  bits += 8;
                }
                if (here_val < 16) {
                  hold >>>= here_bits;
                  bits -= here_bits;
                  state.lens[state.have++] = here_val;
                } else {
                  if (here_val === 16) {
                    n = here_bits + 2;
                    while (bits < n) {
                      if (have === 0) {
                        break inf_leave;
                      }
                      have--;
                      hold += input2[next++] << bits;
                      bits += 8;
                    }
                    hold >>>= here_bits;
                    bits -= here_bits;
                    if (state.have === 0) {
                      strm.msg = "invalid bit length repeat";
                      state.mode = BAD;
                      break;
                    }
                    len = state.lens[state.have - 1];
                    copy = 3 + (hold & 3);
                    hold >>>= 2;
                    bits -= 2;
                  } else if (here_val === 17) {
                    n = here_bits + 3;
                    while (bits < n) {
                      if (have === 0) {
                        break inf_leave;
                      }
                      have--;
                      hold += input2[next++] << bits;
                      bits += 8;
                    }
                    hold >>>= here_bits;
                    bits -= here_bits;
                    len = 0;
                    copy = 3 + (hold & 7);
                    hold >>>= 3;
                    bits -= 3;
                  } else {
                    n = here_bits + 7;
                    while (bits < n) {
                      if (have === 0) {
                        break inf_leave;
                      }
                      have--;
                      hold += input2[next++] << bits;
                      bits += 8;
                    }
                    hold >>>= here_bits;
                    bits -= here_bits;
                    len = 0;
                    copy = 11 + (hold & 127);
                    hold >>>= 7;
                    bits -= 7;
                  }
                  if (state.have + copy > state.nlen + state.ndist) {
                    strm.msg = "invalid bit length repeat";
                    state.mode = BAD;
                    break;
                  }
                  while (copy--) {
                    state.lens[state.have++] = len;
                  }
                }
              }
              if (state.mode === BAD) {
                break;
              }
              if (state.lens[256] === 0) {
                strm.msg = "invalid code -- missing end-of-block";
                state.mode = BAD;
                break;
              }
              state.lenbits = 9;
              opts = { bits: state.lenbits };
              ret = inflate_table(LENS, state.lens, 0, state.nlen, state.lencode, 0, state.work, opts);
              state.lenbits = opts.bits;
              if (ret) {
                strm.msg = "invalid literal/lengths set";
                state.mode = BAD;
                break;
              }
              state.distbits = 6;
              state.distcode = state.distdyn;
              opts = { bits: state.distbits };
              ret = inflate_table(DISTS, state.lens, state.nlen, state.ndist, state.distcode, 0, state.work, opts);
              state.distbits = opts.bits;
              if (ret) {
                strm.msg = "invalid distances set";
                state.mode = BAD;
                break;
              }
              state.mode = LEN_;
              if (flush === Z_TREES) {
                break inf_leave;
              }
            /* falls through */
            case LEN_:
              state.mode = LEN;
            /* falls through */
            case LEN:
              if (have >= 6 && left >= 258) {
                strm.next_out = put;
                strm.avail_out = left;
                strm.next_in = next;
                strm.avail_in = have;
                state.hold = hold;
                state.bits = bits;
                inflate_fast(strm, _out);
                put = strm.next_out;
                output2 = strm.output;
                left = strm.avail_out;
                next = strm.next_in;
                input2 = strm.input;
                have = strm.avail_in;
                hold = state.hold;
                bits = state.bits;
                if (state.mode === TYPE) {
                  state.back = -1;
                }
                break;
              }
              state.back = 0;
              for (; ; ) {
                here = state.lencode[hold & (1 << state.lenbits) - 1];
                here_bits = here >>> 24;
                here_op = here >>> 16 & 255;
                here_val = here & 65535;
                if (here_bits <= bits) {
                  break;
                }
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              if (here_op && (here_op & 240) === 0) {
                last_bits = here_bits;
                last_op = here_op;
                last_val = here_val;
                for (; ; ) {
                  here = state.lencode[last_val + ((hold & (1 << last_bits + last_op) - 1) >> last_bits)];
                  here_bits = here >>> 24;
                  here_op = here >>> 16 & 255;
                  here_val = here & 65535;
                  if (last_bits + here_bits <= bits) {
                    break;
                  }
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input2[next++] << bits;
                  bits += 8;
                }
                hold >>>= last_bits;
                bits -= last_bits;
                state.back += last_bits;
              }
              hold >>>= here_bits;
              bits -= here_bits;
              state.back += here_bits;
              state.length = here_val;
              if (here_op === 0) {
                state.mode = LIT;
                break;
              }
              if (here_op & 32) {
                state.back = -1;
                state.mode = TYPE;
                break;
              }
              if (here_op & 64) {
                strm.msg = "invalid literal/length code";
                state.mode = BAD;
                break;
              }
              state.extra = here_op & 15;
              state.mode = LENEXT;
            /* falls through */
            case LENEXT:
              if (state.extra) {
                n = state.extra;
                while (bits < n) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input2[next++] << bits;
                  bits += 8;
                }
                state.length += hold & (1 << state.extra) - 1;
                hold >>>= state.extra;
                bits -= state.extra;
                state.back += state.extra;
              }
              state.was = state.length;
              state.mode = DIST;
            /* falls through */
            case DIST:
              for (; ; ) {
                here = state.distcode[hold & (1 << state.distbits) - 1];
                here_bits = here >>> 24;
                here_op = here >>> 16 & 255;
                here_val = here & 65535;
                if (here_bits <= bits) {
                  break;
                }
                if (have === 0) {
                  break inf_leave;
                }
                have--;
                hold += input2[next++] << bits;
                bits += 8;
              }
              if ((here_op & 240) === 0) {
                last_bits = here_bits;
                last_op = here_op;
                last_val = here_val;
                for (; ; ) {
                  here = state.distcode[last_val + ((hold & (1 << last_bits + last_op) - 1) >> last_bits)];
                  here_bits = here >>> 24;
                  here_op = here >>> 16 & 255;
                  here_val = here & 65535;
                  if (last_bits + here_bits <= bits) {
                    break;
                  }
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input2[next++] << bits;
                  bits += 8;
                }
                hold >>>= last_bits;
                bits -= last_bits;
                state.back += last_bits;
              }
              hold >>>= here_bits;
              bits -= here_bits;
              state.back += here_bits;
              if (here_op & 64) {
                strm.msg = "invalid distance code";
                state.mode = BAD;
                break;
              }
              state.offset = here_val;
              state.extra = here_op & 15;
              state.mode = DISTEXT;
            /* falls through */
            case DISTEXT:
              if (state.extra) {
                n = state.extra;
                while (bits < n) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input2[next++] << bits;
                  bits += 8;
                }
                state.offset += hold & (1 << state.extra) - 1;
                hold >>>= state.extra;
                bits -= state.extra;
                state.back += state.extra;
              }
              if (state.offset > state.dmax) {
                strm.msg = "invalid distance too far back";
                state.mode = BAD;
                break;
              }
              state.mode = MATCH;
            /* falls through */
            case MATCH:
              if (left === 0) {
                break inf_leave;
              }
              copy = _out - left;
              if (state.offset > copy) {
                copy = state.offset - copy;
                if (copy > state.whave) {
                  if (state.sane) {
                    strm.msg = "invalid distance too far back";
                    state.mode = BAD;
                    break;
                  }
                }
                if (copy > state.wnext) {
                  copy -= state.wnext;
                  from = state.wsize - copy;
                } else {
                  from = state.wnext - copy;
                }
                if (copy > state.length) {
                  copy = state.length;
                }
                from_source = state.window;
              } else {
                from_source = output2;
                from = put - state.offset;
                copy = state.length;
              }
              if (copy > left) {
                copy = left;
              }
              left -= copy;
              state.length -= copy;
              do {
                output2[put++] = from_source[from++];
              } while (--copy);
              if (state.length === 0) {
                state.mode = LEN;
              }
              break;
            case LIT:
              if (left === 0) {
                break inf_leave;
              }
              output2[put++] = state.length;
              left--;
              state.mode = LEN;
              break;
            case CHECK:
              if (state.wrap) {
                while (bits < 32) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold |= input2[next++] << bits;
                  bits += 8;
                }
                _out -= left;
                strm.total_out += _out;
                state.total += _out;
                if (_out) {
                  strm.adler = state.check = /*UPDATE(state.check, put - _out, _out);*/
                  state.flags ? crc32(state.check, output2, _out, put - _out) : adler32(state.check, output2, _out, put - _out);
                }
                _out = left;
                if ((state.flags ? hold : zswap32(hold)) !== state.check) {
                  strm.msg = "incorrect data check";
                  state.mode = BAD;
                  break;
                }
                hold = 0;
                bits = 0;
              }
              state.mode = LENGTH;
            /* falls through */
            case LENGTH:
              if (state.wrap && state.flags) {
                while (bits < 32) {
                  if (have === 0) {
                    break inf_leave;
                  }
                  have--;
                  hold += input2[next++] << bits;
                  bits += 8;
                }
                if (hold !== (state.total & 4294967295)) {
                  strm.msg = "incorrect length check";
                  state.mode = BAD;
                  break;
                }
                hold = 0;
                bits = 0;
              }
              state.mode = DONE;
            /* falls through */
            case DONE:
              ret = Z_STREAM_END;
              break inf_leave;
            case BAD:
              ret = Z_DATA_ERROR;
              break inf_leave;
            case MEM:
              return Z_MEM_ERROR;
            case SYNC:
            /* falls through */
            default:
              return Z_STREAM_ERROR;
          }
        }
      strm.next_out = put;
      strm.avail_out = left;
      strm.next_in = next;
      strm.avail_in = have;
      state.hold = hold;
      state.bits = bits;
      if (state.wsize || _out !== strm.avail_out && state.mode < BAD && (state.mode < CHECK || flush !== Z_FINISH)) {
        if (updatewindow(strm, strm.output, strm.next_out, _out - strm.avail_out)) {
          state.mode = MEM;
          return Z_MEM_ERROR;
        }
      }
      _in -= strm.avail_in;
      _out -= strm.avail_out;
      strm.total_in += _in;
      strm.total_out += _out;
      state.total += _out;
      if (state.wrap && _out) {
        strm.adler = state.check = /*UPDATE(state.check, strm.next_out - _out, _out);*/
        state.flags ? crc32(state.check, output2, _out, strm.next_out - _out) : adler32(state.check, output2, _out, strm.next_out - _out);
      }
      strm.data_type = state.bits + (state.last ? 64 : 0) + (state.mode === TYPE ? 128 : 0) + (state.mode === LEN_ || state.mode === COPY_ ? 256 : 0);
      if ((_in === 0 && _out === 0 || flush === Z_FINISH) && ret === Z_OK) {
        ret = Z_BUF_ERROR;
      }
      return ret;
    }
    function inflateEnd(strm) {
      if (!strm || !strm.state) {
        return Z_STREAM_ERROR;
      }
      var state = strm.state;
      if (state.window) {
        state.window = null;
      }
      strm.state = null;
      return Z_OK;
    }
    function inflateGetHeader(strm, head) {
      var state;
      if (!strm || !strm.state) {
        return Z_STREAM_ERROR;
      }
      state = strm.state;
      if ((state.wrap & 2) === 0) {
        return Z_STREAM_ERROR;
      }
      state.head = head;
      head.done = false;
      return Z_OK;
    }
    function inflateSetDictionary(strm, dictionary) {
      var dictLength = dictionary.length;
      var state;
      var dictid;
      var ret;
      if (!strm || !strm.state) {
        return Z_STREAM_ERROR;
      }
      state = strm.state;
      if (state.wrap !== 0 && state.mode !== DICT) {
        return Z_STREAM_ERROR;
      }
      if (state.mode === DICT) {
        dictid = 1;
        dictid = adler32(dictid, dictionary, dictLength, 0);
        if (dictid !== state.check) {
          return Z_DATA_ERROR;
        }
      }
      ret = updatewindow(strm, dictionary, dictLength, dictLength);
      if (ret) {
        state.mode = MEM;
        return Z_MEM_ERROR;
      }
      state.havedict = 1;
      return Z_OK;
    }
    exports.inflateReset = inflateReset;
    exports.inflateReset2 = inflateReset2;
    exports.inflateResetKeep = inflateResetKeep;
    exports.inflateInit = inflateInit;
    exports.inflateInit2 = inflateInit2;
    exports.inflate = inflate;
    exports.inflateEnd = inflateEnd;
    exports.inflateGetHeader = inflateGetHeader;
    exports.inflateSetDictionary = inflateSetDictionary;
    exports.inflateInfo = "pako inflate (from Nodeca project)";
  }
});

// node_modules/pako/lib/zlib/constants.js
var require_constants = __commonJS({
  "node_modules/pako/lib/zlib/constants.js"(exports, module) {
    "use strict";
    module.exports = {
      /* Allowed flush values; see deflate() and inflate() below for details */
      Z_NO_FLUSH: 0,
      Z_PARTIAL_FLUSH: 1,
      Z_SYNC_FLUSH: 2,
      Z_FULL_FLUSH: 3,
      Z_FINISH: 4,
      Z_BLOCK: 5,
      Z_TREES: 6,
      /* Return codes for the compression/decompression functions. Negative values
      * are errors, positive values are used for special but normal events.
      */
      Z_OK: 0,
      Z_STREAM_END: 1,
      Z_NEED_DICT: 2,
      Z_ERRNO: -1,
      Z_STREAM_ERROR: -2,
      Z_DATA_ERROR: -3,
      //Z_MEM_ERROR:     -4,
      Z_BUF_ERROR: -5,
      //Z_VERSION_ERROR: -6,
      /* compression levels */
      Z_NO_COMPRESSION: 0,
      Z_BEST_SPEED: 1,
      Z_BEST_COMPRESSION: 9,
      Z_DEFAULT_COMPRESSION: -1,
      Z_FILTERED: 1,
      Z_HUFFMAN_ONLY: 2,
      Z_RLE: 3,
      Z_FIXED: 4,
      Z_DEFAULT_STRATEGY: 0,
      /* Possible values of the data_type field (though see inflate()) */
      Z_BINARY: 0,
      Z_TEXT: 1,
      //Z_ASCII:                1, // = Z_TEXT (deprecated)
      Z_UNKNOWN: 2,
      /* The deflate compression method */
      Z_DEFLATED: 8
      //Z_NULL:                 null // Use -1 or null inline, depending on var type
    };
  }
});

// node_modules/pako/lib/zlib/gzheader.js
var require_gzheader = __commonJS({
  "node_modules/pako/lib/zlib/gzheader.js"(exports, module) {
    "use strict";
    function GZheader() {
      this.text = 0;
      this.time = 0;
      this.xflags = 0;
      this.os = 0;
      this.extra = null;
      this.extra_len = 0;
      this.name = "";
      this.comment = "";
      this.hcrc = 0;
      this.done = false;
    }
    module.exports = GZheader;
  }
});

// node_modules/pako/lib/inflate.js
var require_inflate2 = __commonJS({
  "node_modules/pako/lib/inflate.js"(exports) {
    "use strict";
    var zlib_inflate = require_inflate();
    var utils = require_common();
    var strings = require_strings();
    var c = require_constants();
    var msg = require_messages();
    var ZStream = require_zstream();
    var GZheader = require_gzheader();
    var toString = Object.prototype.toString;
    function Inflate(options) {
      if (!(this instanceof Inflate)) return new Inflate(options);
      this.options = utils.assign({
        chunkSize: 16384,
        windowBits: 0,
        to: ""
      }, options || {});
      var opt = this.options;
      if (opt.raw && opt.windowBits >= 0 && opt.windowBits < 16) {
        opt.windowBits = -opt.windowBits;
        if (opt.windowBits === 0) {
          opt.windowBits = -15;
        }
      }
      if (opt.windowBits >= 0 && opt.windowBits < 16 && !(options && options.windowBits)) {
        opt.windowBits += 32;
      }
      if (opt.windowBits > 15 && opt.windowBits < 48) {
        if ((opt.windowBits & 15) === 0) {
          opt.windowBits |= 15;
        }
      }
      this.err = 0;
      this.msg = "";
      this.ended = false;
      this.chunks = [];
      this.strm = new ZStream();
      this.strm.avail_out = 0;
      var status = zlib_inflate.inflateInit2(
        this.strm,
        opt.windowBits
      );
      if (status !== c.Z_OK) {
        throw new Error(msg[status]);
      }
      this.header = new GZheader();
      zlib_inflate.inflateGetHeader(this.strm, this.header);
      if (opt.dictionary) {
        if (typeof opt.dictionary === "string") {
          opt.dictionary = strings.string2buf(opt.dictionary);
        } else if (toString.call(opt.dictionary) === "[object ArrayBuffer]") {
          opt.dictionary = new Uint8Array(opt.dictionary);
        }
        if (opt.raw) {
          status = zlib_inflate.inflateSetDictionary(this.strm, opt.dictionary);
          if (status !== c.Z_OK) {
            throw new Error(msg[status]);
          }
        }
      }
    }
    Inflate.prototype.push = function(data, mode) {
      var strm = this.strm;
      var chunkSize = this.options.chunkSize;
      var dictionary = this.options.dictionary;
      var status, _mode;
      var next_out_utf8, tail, utf8str;
      var allowBufError = false;
      if (this.ended) {
        return false;
      }
      _mode = mode === ~~mode ? mode : mode === true ? c.Z_FINISH : c.Z_NO_FLUSH;
      if (typeof data === "string") {
        strm.input = strings.binstring2buf(data);
      } else if (toString.call(data) === "[object ArrayBuffer]") {
        strm.input = new Uint8Array(data);
      } else {
        strm.input = data;
      }
      strm.next_in = 0;
      strm.avail_in = strm.input.length;
      do {
        if (strm.avail_out === 0) {
          strm.output = new utils.Buf8(chunkSize);
          strm.next_out = 0;
          strm.avail_out = chunkSize;
        }
        status = zlib_inflate.inflate(strm, c.Z_NO_FLUSH);
        if (status === c.Z_NEED_DICT && dictionary) {
          status = zlib_inflate.inflateSetDictionary(this.strm, dictionary);
        }
        if (status === c.Z_BUF_ERROR && allowBufError === true) {
          status = c.Z_OK;
          allowBufError = false;
        }
        if (status !== c.Z_STREAM_END && status !== c.Z_OK) {
          this.onEnd(status);
          this.ended = true;
          return false;
        }
        if (strm.next_out) {
          if (strm.avail_out === 0 || status === c.Z_STREAM_END || strm.avail_in === 0 && (_mode === c.Z_FINISH || _mode === c.Z_SYNC_FLUSH)) {
            if (this.options.to === "string") {
              next_out_utf8 = strings.utf8border(strm.output, strm.next_out);
              tail = strm.next_out - next_out_utf8;
              utf8str = strings.buf2string(strm.output, next_out_utf8);
              strm.next_out = tail;
              strm.avail_out = chunkSize - tail;
              if (tail) {
                utils.arraySet(strm.output, strm.output, next_out_utf8, tail, 0);
              }
              this.onData(utf8str);
            } else {
              this.onData(utils.shrinkBuf(strm.output, strm.next_out));
            }
          }
        }
        if (strm.avail_in === 0 && strm.avail_out === 0) {
          allowBufError = true;
        }
      } while ((strm.avail_in > 0 || strm.avail_out === 0) && status !== c.Z_STREAM_END);
      if (status === c.Z_STREAM_END) {
        _mode = c.Z_FINISH;
      }
      if (_mode === c.Z_FINISH) {
        status = zlib_inflate.inflateEnd(this.strm);
        this.onEnd(status);
        this.ended = true;
        return status === c.Z_OK;
      }
      if (_mode === c.Z_SYNC_FLUSH) {
        this.onEnd(c.Z_OK);
        strm.avail_out = 0;
        return true;
      }
      return true;
    };
    Inflate.prototype.onData = function(chunk) {
      this.chunks.push(chunk);
    };
    Inflate.prototype.onEnd = function(status) {
      if (status === c.Z_OK) {
        if (this.options.to === "string") {
          this.result = this.chunks.join("");
        } else {
          this.result = utils.flattenChunks(this.chunks);
        }
      }
      this.chunks = [];
      this.err = status;
      this.msg = this.strm.msg;
    };
    function inflate(input2, options) {
      var inflator = new Inflate(options);
      inflator.push(input2, true);
      if (inflator.err) {
        throw inflator.msg || msg[inflator.err];
      }
      return inflator.result;
    }
    function inflateRaw(input2, options) {
      options = options || {};
      options.raw = true;
      return inflate(input2, options);
    }
    exports.Inflate = Inflate;
    exports.inflate = inflate;
    exports.inflateRaw = inflateRaw;
    exports.ungzip = inflate;
  }
});

// node_modules/pako/index.js
var require_pako = __commonJS({
  "node_modules/pako/index.js"(exports, module) {
    "use strict";
    var assign = require_common().assign;
    var deflate = require_deflate2();
    var inflate = require_inflate2();
    var constants = require_constants();
    var pako = {};
    assign(pako, deflate, inflate, constants);
    module.exports = pako;
  }
});

// node_modules/pify/index.js
var require_pify = __commonJS({
  "node_modules/pify/index.js"(exports, module) {
    "use strict";
    var processFn = (fn, options) => function(...args) {
      const P = options.promiseModule;
      return new P((resolve, reject) => {
        if (options.multiArgs) {
          args.push((...result) => {
            if (options.errorFirst) {
              if (result[0]) {
                reject(result);
              } else {
                result.shift();
                resolve(result);
              }
            } else {
              resolve(result);
            }
          });
        } else if (options.errorFirst) {
          args.push((error, result) => {
            if (error) {
              reject(error);
            } else {
              resolve(result);
            }
          });
        } else {
          args.push(resolve);
        }
        fn.apply(this, args);
      });
    };
    module.exports = (input2, options) => {
      options = Object.assign({
        exclude: [/.+(Sync|Stream)$/],
        errorFirst: true,
        promiseModule: Promise
      }, options);
      const objType = typeof input2;
      if (!(input2 !== null && (objType === "object" || objType === "function"))) {
        throw new TypeError(`Expected \`input\` to be a \`Function\` or \`Object\`, got \`${input2 === null ? "null" : objType}\``);
      }
      const filter = (key) => {
        const match = (pattern) => typeof pattern === "string" ? key === pattern : pattern.test(key);
        return options.include ? options.include.some(match) : !options.exclude.some(match);
      };
      let ret;
      if (objType === "function") {
        ret = function(...args) {
          return options.excludeMain ? input2(...args) : processFn(input2, options).apply(this, args);
        };
      } else {
        ret = Object.create(Object.getPrototypeOf(input2));
      }
      for (const key in input2) {
        const property = input2[key];
        ret[key] = typeof property === "function" && filter(key) ? processFn(property, options) : property;
      }
      return ret;
    };
  }
});

// node_modules/ignore/index.js
var require_ignore = __commonJS({
  "node_modules/ignore/index.js"(exports, module) {
    function makeArray(subject) {
      return Array.isArray(subject) ? subject : [subject];
    }
    var EMPTY = "";
    var SPACE = " ";
    var ESCAPE = "\\";
    var REGEX_TEST_BLANK_LINE = /^\s+$/;
    var REGEX_INVALID_TRAILING_BACKSLASH = /(?:[^\\]|^)\\$/;
    var REGEX_REPLACE_LEADING_EXCAPED_EXCLAMATION = /^\\!/;
    var REGEX_REPLACE_LEADING_EXCAPED_HASH = /^\\#/;
    var REGEX_SPLITALL_CRLF = /\r?\n/g;
    var REGEX_TEST_INVALID_PATH = /^\.*\/|^\.+$/;
    var SLASH = "/";
    var TMP_KEY_IGNORE = "node-ignore";
    if (typeof Symbol !== "undefined") {
      TMP_KEY_IGNORE = Symbol.for("node-ignore");
    }
    var KEY_IGNORE = TMP_KEY_IGNORE;
    var define2 = (object, key, value) => Object.defineProperty(object, key, { value });
    var REGEX_REGEXP_RANGE = /([0-z])-([0-z])/g;
    var RETURN_FALSE = () => false;
    var sanitizeRange = (range) => range.replace(
      REGEX_REGEXP_RANGE,
      (match, from, to) => from.charCodeAt(0) <= to.charCodeAt(0) ? match : EMPTY
    );
    var cleanRangeBackSlash = (slashes) => {
      const { length } = slashes;
      return slashes.slice(0, length - length % 2);
    };
    var REPLACERS = [
      [
        // remove BOM
        // TODO:
        // Other similar zero-width characters?
        /^\uFEFF/,
        () => EMPTY
      ],
      // > Trailing spaces are ignored unless they are quoted with backslash ("\")
      [
        // (a\ ) -> (a )
        // (a  ) -> (a)
        // (a ) -> (a)
        // (a \ ) -> (a  )
        /((?:\\\\)*?)(\\?\s+)$/,
        (_, m1, m2) => m1 + (m2.indexOf("\\") === 0 ? SPACE : EMPTY)
      ],
      // replace (\ ) with ' '
      // (\ ) -> ' '
      // (\\ ) -> '\\ '
      // (\\\ ) -> '\\ '
      [
        /(\\+?)\s/g,
        (_, m1) => {
          const { length } = m1;
          return m1.slice(0, length - length % 2) + SPACE;
        }
      ],
      // Escape metacharacters
      // which is written down by users but means special for regular expressions.
      // > There are 12 characters with special meanings:
      // > - the backslash \,
      // > - the caret ^,
      // > - the dollar sign $,
      // > - the period or dot .,
      // > - the vertical bar or pipe symbol |,
      // > - the question mark ?,
      // > - the asterisk or star *,
      // > - the plus sign +,
      // > - the opening parenthesis (,
      // > - the closing parenthesis ),
      // > - and the opening square bracket [,
      // > - the opening curly brace {,
      // > These special characters are often called "metacharacters".
      [
        /[\\$.|*+(){^]/g,
        (match) => `\\${match}`
      ],
      [
        // > a question mark (?) matches a single character
        /(?!\\)\?/g,
        () => "[^/]"
      ],
      // leading slash
      [
        // > A leading slash matches the beginning of the pathname.
        // > For example, "/*.c" matches "cat-file.c" but not "mozilla-sha1/sha1.c".
        // A leading slash matches the beginning of the pathname
        /^\//,
        () => "^"
      ],
      // replace special metacharacter slash after the leading slash
      [
        /\//g,
        () => "\\/"
      ],
      [
        // > A leading "**" followed by a slash means match in all directories.
        // > For example, "**/foo" matches file or directory "foo" anywhere,
        // > the same as pattern "foo".
        // > "**/foo/bar" matches file or directory "bar" anywhere that is directly
        // >   under directory "foo".
        // Notice that the '*'s have been replaced as '\\*'
        /^\^*\\\*\\\*\\\//,
        // '**/foo' <-> 'foo'
        () => "^(?:.*\\/)?"
      ],
      // starting
      [
        // there will be no leading '/'
        //   (which has been replaced by section "leading slash")
        // If starts with '**', adding a '^' to the regular expression also works
        /^(?=[^^])/,
        function startingReplacer() {
          return !/\/(?!$)/.test(this) ? "(?:^|\\/)" : "^";
        }
      ],
      // two globstars
      [
        // Use lookahead assertions so that we could match more than one `'/**'`
        /\\\/\\\*\\\*(?=\\\/|$)/g,
        // Zero, one or several directories
        // should not use '*', or it will be replaced by the next replacer
        // Check if it is not the last `'/**'`
        (_, index, str) => index + 6 < str.length ? "(?:\\/[^\\/]+)*" : "\\/.+"
      ],
      // normal intermediate wildcards
      [
        // Never replace escaped '*'
        // ignore rule '\*' will match the path '*'
        // 'abc.*/' -> go
        // 'abc.*'  -> skip this rule,
        //    coz trailing single wildcard will be handed by [trailing wildcard]
        /(^|[^\\]+)(\\\*)+(?=.+)/g,
        // '*.js' matches '.js'
        // '*.js' doesn't match 'abc'
        (_, p1, p2) => {
          const unescaped = p2.replace(/\\\*/g, "[^\\/]*");
          return p1 + unescaped;
        }
      ],
      [
        // unescape, revert step 3 except for back slash
        // For example, if a user escape a '\\*',
        // after step 3, the result will be '\\\\\\*'
        /\\\\\\(?=[$.|*+(){^])/g,
        () => ESCAPE
      ],
      [
        // '\\\\' -> '\\'
        /\\\\/g,
        () => ESCAPE
      ],
      [
        // > The range notation, e.g. [a-zA-Z],
        // > can be used to match one of the characters in a range.
        // `\` is escaped by step 3
        /(\\)?\[([^\]/]*?)(\\*)($|\])/g,
        (match, leadEscape, range, endEscape, close) => leadEscape === ESCAPE ? `\\[${range}${cleanRangeBackSlash(endEscape)}${close}` : close === "]" ? endEscape.length % 2 === 0 ? `[${sanitizeRange(range)}${endEscape}]` : "[]" : "[]"
      ],
      // ending
      [
        // 'js' will not match 'js.'
        // 'ab' will not match 'abc'
        /(?:[^*])$/,
        // WTF!
        // https://git-scm.com/docs/gitignore
        // changes in [2.22.1](https://git-scm.com/docs/gitignore/2.22.1)
        // which re-fixes #24, #38
        // > If there is a separator at the end of the pattern then the pattern
        // > will only match directories, otherwise the pattern can match both
        // > files and directories.
        // 'js*' will not match 'a.js'
        // 'js/' will not match 'a.js'
        // 'js' will match 'a.js' and 'a.js/'
        (match) => /\/$/.test(match) ? `${match}$` : `${match}(?=$|\\/$)`
      ],
      // trailing wildcard
      [
        /(\^|\\\/)?\\\*$/,
        (_, p1) => {
          const prefix = p1 ? `${p1}[^/]+` : "[^/]*";
          return `${prefix}(?=$|\\/$)`;
        }
      ]
    ];
    var regexCache = /* @__PURE__ */ Object.create(null);
    var makeRegex = (pattern, ignoreCase) => {
      let source = regexCache[pattern];
      if (!source) {
        source = REPLACERS.reduce(
          (prev, [matcher, replacer]) => prev.replace(matcher, replacer.bind(pattern)),
          pattern
        );
        regexCache[pattern] = source;
      }
      return ignoreCase ? new RegExp(source, "i") : new RegExp(source);
    };
    var isString = (subject) => typeof subject === "string";
    var checkPattern = (pattern) => pattern && isString(pattern) && !REGEX_TEST_BLANK_LINE.test(pattern) && !REGEX_INVALID_TRAILING_BACKSLASH.test(pattern) && pattern.indexOf("#") !== 0;
    var splitPattern = (pattern) => pattern.split(REGEX_SPLITALL_CRLF);
    var IgnoreRule = class {
      constructor(origin, pattern, negative, regex) {
        this.origin = origin;
        this.pattern = pattern;
        this.negative = negative;
        this.regex = regex;
      }
    };
    var createRule = (pattern, ignoreCase) => {
      const origin = pattern;
      let negative = false;
      if (pattern.indexOf("!") === 0) {
        negative = true;
        pattern = pattern.substr(1);
      }
      pattern = pattern.replace(REGEX_REPLACE_LEADING_EXCAPED_EXCLAMATION, "!").replace(REGEX_REPLACE_LEADING_EXCAPED_HASH, "#");
      const regex = makeRegex(pattern, ignoreCase);
      return new IgnoreRule(
        origin,
        pattern,
        negative,
        regex
      );
    };
    var throwError = (message, Ctor) => {
      throw new Ctor(message);
    };
    var checkPath = (path3, originalPath, doThrow) => {
      if (!isString(path3)) {
        return doThrow(
          `path must be a string, but got \`${originalPath}\``,
          TypeError
        );
      }
      if (!path3) {
        return doThrow(`path must not be empty`, TypeError);
      }
      if (checkPath.isNotRelative(path3)) {
        const r = "`path.relative()`d";
        return doThrow(
          `path should be a ${r} string, but got "${originalPath}"`,
          RangeError
        );
      }
      return true;
    };
    var isNotRelative = (path3) => REGEX_TEST_INVALID_PATH.test(path3);
    checkPath.isNotRelative = isNotRelative;
    checkPath.convert = (p) => p;
    var Ignore = class {
      constructor({
        ignorecase = true,
        ignoreCase = ignorecase,
        allowRelativePaths = false
      } = {}) {
        define2(this, KEY_IGNORE, true);
        this._rules = [];
        this._ignoreCase = ignoreCase;
        this._allowRelativePaths = allowRelativePaths;
        this._initCache();
      }
      _initCache() {
        this._ignoreCache = /* @__PURE__ */ Object.create(null);
        this._testCache = /* @__PURE__ */ Object.create(null);
      }
      _addPattern(pattern) {
        if (pattern && pattern[KEY_IGNORE]) {
          this._rules = this._rules.concat(pattern._rules);
          this._added = true;
          return;
        }
        if (checkPattern(pattern)) {
          const rule = createRule(pattern, this._ignoreCase);
          this._added = true;
          this._rules.push(rule);
        }
      }
      // @param {Array<string> | string | Ignore} pattern
      add(pattern) {
        this._added = false;
        makeArray(
          isString(pattern) ? splitPattern(pattern) : pattern
        ).forEach(this._addPattern, this);
        if (this._added) {
          this._initCache();
        }
        return this;
      }
      // legacy
      addPattern(pattern) {
        return this.add(pattern);
      }
      //          |           ignored : unignored
      // negative |   0:0   |   0:1   |   1:0   |   1:1
      // -------- | ------- | ------- | ------- | --------
      //     0    |  TEST   |  TEST   |  SKIP   |    X
      //     1    |  TESTIF |  SKIP   |  TEST   |    X
      // - SKIP: always skip
      // - TEST: always test
      // - TESTIF: only test if checkUnignored
      // - X: that never happen
      // @param {boolean} whether should check if the path is unignored,
      //   setting `checkUnignored` to `false` could reduce additional
      //   path matching.
      // @returns {TestResult} true if a file is ignored
      _testOne(path3, checkUnignored) {
        let ignored = false;
        let unignored = false;
        this._rules.forEach((rule) => {
          const { negative } = rule;
          if (unignored === negative && ignored !== unignored || negative && !ignored && !unignored && !checkUnignored) {
            return;
          }
          const matched = rule.regex.test(path3);
          if (matched) {
            ignored = !negative;
            unignored = negative;
          }
        });
        return {
          ignored,
          unignored
        };
      }
      // @returns {TestResult}
      _test(originalPath, cache, checkUnignored, slices) {
        const path3 = originalPath && checkPath.convert(originalPath);
        checkPath(
          path3,
          originalPath,
          this._allowRelativePaths ? RETURN_FALSE : throwError
        );
        return this._t(path3, cache, checkUnignored, slices);
      }
      _t(path3, cache, checkUnignored, slices) {
        if (path3 in cache) {
          return cache[path3];
        }
        if (!slices) {
          slices = path3.split(SLASH);
        }
        slices.pop();
        if (!slices.length) {
          return cache[path3] = this._testOne(path3, checkUnignored);
        }
        const parent = this._t(
          slices.join(SLASH) + SLASH,
          cache,
          checkUnignored,
          slices
        );
        return cache[path3] = parent.ignored ? parent : this._testOne(path3, checkUnignored);
      }
      ignores(path3) {
        return this._test(path3, this._ignoreCache, false).ignored;
      }
      createFilter() {
        return (path3) => !this.ignores(path3);
      }
      filter(paths) {
        return makeArray(paths).filter(this.createFilter());
      }
      // @returns {TestResult}
      test(path3) {
        return this._test(path3, this._testCache, true);
      }
    };
    var factory = (options) => new Ignore(options);
    var isPathValid = (path3) => checkPath(path3 && checkPath.convert(path3), path3, RETURN_FALSE);
    factory.isPathValid = isPathValid;
    factory.default = factory;
    module.exports = factory;
    if (
      // Detect `process` so that it can run in browsers.
      typeof process !== "undefined" && (process.env && process.env.IGNORE_TEST_WIN32 || process.platform === "win32")
    ) {
      const makePosix = (str) => /^\\\\\?\\/.test(str) || /["<>|\u0000-\u001F]+/u.test(str) ? str : str.replace(/\\/g, "/");
      checkPath.convert = makePosix;
      const REGIX_IS_WINDOWS_PATH_ABSOLUTE = /^[a-z]:\//i;
      checkPath.isNotRelative = (path3) => REGIX_IS_WINDOWS_PATH_ABSOLUTE.test(path3) || isNotRelative(path3);
    }
  }
});

// node_modules/clean-git-ref/lib/index.js
var require_lib2 = __commonJS({
  "node_modules/clean-git-ref/lib/index.js"(exports, module) {
    "use strict";
    function escapeRegExp(string) {
      return string.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    }
    function replaceAll(str, search, replacement) {
      search = search instanceof RegExp ? search : new RegExp(escapeRegExp(search), "g");
      return str.replace(search, replacement);
    }
    var CleanGitRef = {
      clean: function clean(value) {
        if (typeof value !== "string") {
          throw new Error("Expected a string, received: " + value);
        }
        value = replaceAll(value, "./", "/");
        value = replaceAll(value, "..", ".");
        value = replaceAll(value, " ", "-");
        value = replaceAll(value, /^[~^:?*\\\-]/g, "");
        value = replaceAll(value, /[~^:?*\\]/g, "-");
        value = replaceAll(value, /[~^:?*\\\-]$/g, "");
        value = replaceAll(value, "@{", "-");
        value = replaceAll(value, /\.$/g, "");
        value = replaceAll(value, /\/$/g, "");
        value = replaceAll(value, /\.lock$/g, "");
        return value;
      }
    };
    module.exports = CleanGitRef;
  }
});

// node_modules/isomorphic-git/node_modules/diff3/onp.js
var require_onp = __commonJS({
  "node_modules/isomorphic-git/node_modules/diff3/onp.js"(exports, module) {
    module.exports = function(a_, b_) {
      var a = a_, b = b_, m = a.length, n = b.length, reverse = false, ed = null, offset = m + 1, path3 = [], pathposi = [], ses = [], lcs = "", SES_DELETE = -1, SES_COMMON = 0, SES_ADD = 1;
      var tmp1, tmp2;
      var init2 = function() {
        if (m >= n) {
          tmp1 = a;
          tmp2 = m;
          a = b;
          b = tmp1;
          m = n;
          n = tmp2;
          reverse = true;
          offset = m + 1;
        }
      };
      var P = function(x, y, k) {
        return {
          "x": x,
          "y": y,
          "k": k
        };
      };
      var seselem = function(elem, t) {
        return {
          "elem": elem,
          "t": t
        };
      };
      var snake = function(k, p, pp) {
        var r, x, y;
        if (p > pp) {
          r = path3[k - 1 + offset];
        } else {
          r = path3[k + 1 + offset];
        }
        y = Math.max(p, pp);
        x = y - k;
        while (x < m && y < n && a[x] === b[y]) {
          ++x;
          ++y;
        }
        path3[k + offset] = pathposi.length;
        pathposi[pathposi.length] = new P(x, y, r);
        return y;
      };
      var recordseq = function(epc) {
        var x_idx, y_idx, px_idx, py_idx, i;
        x_idx = y_idx = 1;
        px_idx = py_idx = 0;
        for (i = epc.length - 1; i >= 0; --i) {
          while (px_idx < epc[i].x || py_idx < epc[i].y) {
            if (epc[i].y - epc[i].x > py_idx - px_idx) {
              if (reverse) {
                ses[ses.length] = new seselem(b[py_idx], SES_DELETE);
              } else {
                ses[ses.length] = new seselem(b[py_idx], SES_ADD);
              }
              ++y_idx;
              ++py_idx;
            } else if (epc[i].y - epc[i].x < py_idx - px_idx) {
              if (reverse) {
                ses[ses.length] = new seselem(a[px_idx], SES_ADD);
              } else {
                ses[ses.length] = new seselem(a[px_idx], SES_DELETE);
              }
              ++x_idx;
              ++px_idx;
            } else {
              ses[ses.length] = new seselem(a[px_idx], SES_COMMON);
              lcs += a[px_idx];
              ++x_idx;
              ++y_idx;
              ++px_idx;
              ++py_idx;
            }
          }
        }
      };
      init2();
      return {
        SES_DELETE: -1,
        SES_COMMON: 0,
        SES_ADD: 1,
        editdistance: function() {
          return ed;
        },
        getlcs: function() {
          return lcs;
        },
        getses: function() {
          return ses;
        },
        compose: function() {
          var delta, size, fp, p, r, epc, i, k;
          delta = n - m;
          size = m + n + 3;
          fp = {};
          for (i = 0; i < size; ++i) {
            fp[i] = -1;
            path3[i] = -1;
          }
          p = -1;
          do {
            ++p;
            for (k = -p; k <= delta - 1; ++k) {
              fp[k + offset] = snake(k, fp[k - 1 + offset] + 1, fp[k + 1 + offset]);
            }
            for (k = delta + p; k >= delta + 1; --k) {
              fp[k + offset] = snake(k, fp[k - 1 + offset] + 1, fp[k + 1 + offset]);
            }
            fp[delta + offset] = snake(delta, fp[delta - 1 + offset] + 1, fp[delta + 1 + offset]);
          } while (fp[delta + offset] !== n);
          ed = delta + 2 * p;
          r = path3[delta + offset];
          epc = [];
          while (r !== -1) {
            epc[epc.length] = new P(pathposi[r].x, pathposi[r].y, null);
            r = pathposi[r].k;
          }
          recordseq(epc);
        }
      };
    };
  }
});

// node_modules/isomorphic-git/node_modules/diff3/diff3.js
var require_diff3 = __commonJS({
  "node_modules/isomorphic-git/node_modules/diff3/diff3.js"(exports, module) {
    var onp = require_onp();
    function longestCommonSubsequence(file1, file2) {
      var diff2 = new onp(file1, file2);
      diff2.compose();
      var ses = diff2.getses();
      var root;
      var prev;
      var file1RevIdx = file1.length - 1, file2RevIdx = file2.length - 1;
      for (var i = ses.length - 1; i >= 0; --i) {
        if (ses[i].t === diff2.SES_COMMON) {
          if (prev) {
            prev.chain = {
              file1index: file1RevIdx,
              file2index: file2RevIdx,
              chain: null
            };
            prev = prev.chain;
          } else {
            root = {
              file1index: file1RevIdx,
              file2index: file2RevIdx,
              chain: null
            };
            prev = root;
          }
          file1RevIdx--;
          file2RevIdx--;
        } else if (ses[i].t === diff2.SES_DELETE) {
          file1RevIdx--;
        } else if (ses[i].t === diff2.SES_ADD) {
          file2RevIdx--;
        }
      }
      var tail = {
        file1index: -1,
        file2index: -1,
        chain: null
      };
      if (!prev) {
        return tail;
      }
      prev.chain = tail;
      return root;
    }
    function diffIndices2(file1, file2) {
      var result = [];
      var tail1 = file1.length;
      var tail2 = file2.length;
      for (var candidate = longestCommonSubsequence(file1, file2); candidate !== null; candidate = candidate.chain) {
        var mismatchLength1 = tail1 - candidate.file1index - 1;
        var mismatchLength2 = tail2 - candidate.file2index - 1;
        tail1 = candidate.file1index;
        tail2 = candidate.file2index;
        if (mismatchLength1 || mismatchLength2) {
          result.push({
            file1: [tail1 + 1, mismatchLength1],
            file2: [tail2 + 1, mismatchLength2]
          });
        }
      }
      result.reverse();
      return result;
    }
    function diff3MergeIndices(a, o, b) {
      var i;
      var m1 = diffIndices2(o, a);
      var m2 = diffIndices2(o, b);
      var hunks = [];
      function addHunk(h, side2) {
        hunks.push([h.file1[0], side2, h.file1[1], h.file2[0], h.file2[1]]);
      }
      for (i = 0; i < m1.length; i++) {
        addHunk(m1[i], 0);
      }
      for (i = 0; i < m2.length; i++) {
        addHunk(m2[i], 2);
      }
      hunks.sort(function(x, y) {
        return x[0] - y[0];
      });
      var result = [];
      var commonOffset = 0;
      function copyCommon(targetOffset) {
        if (targetOffset > commonOffset) {
          result.push([1, commonOffset, targetOffset - commonOffset]);
          commonOffset = targetOffset;
        }
      }
      for (var hunkIndex = 0; hunkIndex < hunks.length; hunkIndex++) {
        var firstHunkIndex = hunkIndex;
        var hunk = hunks[hunkIndex];
        var regionLhs = hunk[0];
        var regionRhs = regionLhs + hunk[2];
        while (hunkIndex < hunks.length - 1) {
          var maybeOverlapping = hunks[hunkIndex + 1];
          var maybeLhs = maybeOverlapping[0];
          if (maybeLhs > regionRhs) break;
          regionRhs = Math.max(regionRhs, maybeLhs + maybeOverlapping[2]);
          hunkIndex++;
        }
        copyCommon(regionLhs);
        if (firstHunkIndex == hunkIndex) {
          if (hunk[4] > 0) {
            result.push([hunk[1], hunk[3], hunk[4]]);
          }
        } else {
          var regions = {
            0: [a.length, -1, o.length, -1],
            2: [b.length, -1, o.length, -1]
          };
          for (i = firstHunkIndex; i <= hunkIndex; i++) {
            hunk = hunks[i];
            var side = hunk[1];
            var r = regions[side];
            var oLhs = hunk[0];
            var oRhs = oLhs + hunk[2];
            var abLhs = hunk[3];
            var abRhs = abLhs + hunk[4];
            r[0] = Math.min(abLhs, r[0]);
            r[1] = Math.max(abRhs, r[1]);
            r[2] = Math.min(oLhs, r[2]);
            r[3] = Math.max(oRhs, r[3]);
          }
          var aLhs = regions[0][0] + (regionLhs - regions[0][2]);
          var aRhs = regions[0][1] + (regionRhs - regions[0][3]);
          var bLhs = regions[2][0] + (regionLhs - regions[2][2]);
          var bRhs = regions[2][1] + (regionRhs - regions[2][3]);
          result.push([
            -1,
            aLhs,
            aRhs - aLhs,
            regionLhs,
            regionRhs - regionLhs,
            bLhs,
            bRhs - bLhs
          ]);
        }
        commonOffset = regionRhs;
      }
      copyCommon(o.length);
      return result;
    }
    function diff3Merge2(a, o, b) {
      var result = [];
      var files = [a, o, b];
      var indices = diff3MergeIndices(a, o, b);
      var okLines = [];
      function flushOk() {
        if (okLines.length) {
          result.push({
            ok: okLines
          });
        }
        okLines = [];
      }
      function pushOk(xs) {
        for (var j = 0; j < xs.length; j++) {
          okLines.push(xs[j]);
        }
      }
      function isTrueConflict(rec) {
        if (rec[2] != rec[6]) return true;
        var aoff = rec[1];
        var boff = rec[5];
        for (var j = 0; j < rec[2]; j++) {
          if (a[j + aoff] != b[j + boff]) return true;
        }
        return false;
      }
      for (var i = 0; i < indices.length; i++) {
        var x = indices[i];
        var side = x[0];
        if (side == -1) {
          if (!isTrueConflict(x)) {
            pushOk(files[0].slice(x[1], x[1] + x[2]));
          } else {
            flushOk();
            result.push({
              conflict: {
                a: a.slice(x[1], x[1] + x[2]),
                aIndex: x[1],
                o: o.slice(x[3], x[3] + x[4]),
                oIndex: x[3],
                b: b.slice(x[5], x[5] + x[6]),
                bIndex: x[5]
              }
            });
          }
        } else {
          pushOk(files[side].slice(x[1], x[1] + x[2]));
        }
      }
      flushOk();
      return result;
    }
    module.exports = diff3Merge2;
  }
});

// node_modules/isomorphic-git/index.cjs
var require_isomorphic_git = __commonJS({
  "node_modules/isomorphic-git/index.cjs"(exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    function _interopDefault(ex) {
      return ex && typeof ex === "object" && "default" in ex ? ex["default"] : ex;
    }
    var AsyncLock = _interopDefault(require_async_lock());
    var Hash = _interopDefault(require_sha1());
    var crc32 = _interopDefault(require_crc32());
    var pako = _interopDefault(require_pako());
    var crypto$1 = __require("crypto");
    var pify = _interopDefault(require_pify());
    var ignore = _interopDefault(require_ignore());
    var cleanGitRef = _interopDefault(require_lib2());
    var diff3Merge2 = _interopDefault(require_diff3());
    var BaseError = class _BaseError extends Error {
      constructor(message) {
        super(message);
        this.caller = "";
      }
      toJSON() {
        return {
          code: this.code,
          data: this.data,
          caller: this.caller,
          message: this.message,
          stack: this.stack
        };
      }
      fromJSON(json) {
        const e = new _BaseError(json.message);
        e.code = json.code;
        e.data = json.data;
        e.caller = json.caller;
        e.stack = json.stack;
        return e;
      }
      get isIsomorphicGitError() {
        return true;
      }
    };
    var UnmergedPathsError = class _UnmergedPathsError extends BaseError {
      /**
       * @param {Array<string>} filepaths
       */
      constructor(filepaths) {
        super(
          `Modifying the index is not possible because you have unmerged files: ${filepaths.toString}. Fix them up in the work tree, and then use 'git add/rm as appropriate to mark resolution and make a commit.`
        );
        this.code = this.name = _UnmergedPathsError.code;
        this.data = { filepaths };
      }
    };
    UnmergedPathsError.code = "UnmergedPathsError";
    var InternalError = class _InternalError extends BaseError {
      /**
       * @param {string} message
       */
      constructor(message) {
        super(
          `An internal error caused this command to fail.

If you're not a developer, report the bug to the developers of the application you're using. If this is a bug in isomorphic-git then you should create a proper bug yourselves. The bug should include a minimal reproduction and details about the version and environment.

Please file a bug report at https://github.com/isomorphic-git/isomorphic-git/issues with this error message: ${message}`
        );
        this.code = this.name = _InternalError.code;
        this.data = { message };
      }
    };
    InternalError.code = "InternalError";
    var UnsafeFilepathError = class _UnsafeFilepathError extends BaseError {
      /**
       * @param {string} filepath
       */
      constructor(filepath) {
        super(`The filepath "${filepath}" contains unsafe character sequences`);
        this.code = this.name = _UnsafeFilepathError.code;
        this.data = { filepath };
      }
    };
    UnsafeFilepathError.code = "UnsafeFilepathError";
    var BufferCursor = class {
      constructor(buffer) {
        this.buffer = buffer;
        this._start = 0;
      }
      eof() {
        return this._start >= this.buffer.length;
      }
      tell() {
        return this._start;
      }
      seek(n) {
        this._start = n;
      }
      slice(n) {
        const r = this.buffer.slice(this._start, this._start + n);
        this._start += n;
        return r;
      }
      toString(enc, length) {
        const r = this.buffer.toString(enc, this._start, this._start + length);
        this._start += length;
        return r;
      }
      write(value, length, enc) {
        const r = this.buffer.write(value, this._start, length, enc);
        this._start += length;
        return r;
      }
      copy(source, start, end) {
        const r = source.copy(this.buffer, this._start, start, end);
        this._start += r;
        return r;
      }
      readUInt8() {
        const r = this.buffer.readUInt8(this._start);
        this._start += 1;
        return r;
      }
      writeUInt8(value) {
        const r = this.buffer.writeUInt8(value, this._start);
        this._start += 1;
        return r;
      }
      readUInt16BE() {
        const r = this.buffer.readUInt16BE(this._start);
        this._start += 2;
        return r;
      }
      writeUInt16BE(value) {
        const r = this.buffer.writeUInt16BE(value, this._start);
        this._start += 2;
        return r;
      }
      readUInt32BE() {
        const r = this.buffer.readUInt32BE(this._start);
        this._start += 4;
        return r;
      }
      writeUInt32BE(value) {
        const r = this.buffer.writeUInt32BE(value, this._start);
        this._start += 4;
        return r;
      }
    };
    function compareStrings(a, b) {
      return -(a < b) || +(a > b);
    }
    function comparePath(a, b) {
      return compareStrings(a.path, b.path);
    }
    function normalizeMode(mode) {
      let type = mode > 0 ? mode >> 12 : 0;
      if (type !== 4 && type !== 8 && type !== 10 && type !== 14) {
        type = 8;
      }
      let permissions = mode & 511;
      if (permissions & 73) {
        permissions = 493;
      } else {
        permissions = 420;
      }
      if (type !== 8) permissions = 0;
      return (type << 12) + permissions;
    }
    var MAX_UINT32 = 2 ** 32;
    function SecondsNanoseconds(givenSeconds, givenNanoseconds, milliseconds, date) {
      if (givenSeconds !== void 0 && givenNanoseconds !== void 0) {
        return [givenSeconds, givenNanoseconds];
      }
      if (milliseconds === void 0) {
        milliseconds = date.valueOf();
      }
      const seconds = Math.floor(milliseconds / 1e3);
      const nanoseconds = (milliseconds - seconds * 1e3) * 1e6;
      return [seconds, nanoseconds];
    }
    function normalizeStats(e) {
      const [ctimeSeconds, ctimeNanoseconds] = SecondsNanoseconds(
        e.ctimeSeconds,
        e.ctimeNanoseconds,
        e.ctimeMs,
        e.ctime
      );
      const [mtimeSeconds, mtimeNanoseconds] = SecondsNanoseconds(
        e.mtimeSeconds,
        e.mtimeNanoseconds,
        e.mtimeMs,
        e.mtime
      );
      return {
        ctimeSeconds: ctimeSeconds % MAX_UINT32,
        ctimeNanoseconds: ctimeNanoseconds % MAX_UINT32,
        mtimeSeconds: mtimeSeconds % MAX_UINT32,
        mtimeNanoseconds: mtimeNanoseconds % MAX_UINT32,
        dev: e.dev % MAX_UINT32,
        ino: e.ino % MAX_UINT32,
        mode: normalizeMode(e.mode % MAX_UINT32),
        uid: e.uid % MAX_UINT32,
        gid: e.gid % MAX_UINT32,
        // size of -1 happens over a BrowserFS HTTP Backend that doesn't serve Content-Length headers
        // (like the Karma webserver) because BrowserFS HTTP Backend uses HTTP HEAD requests to do fs.stat
        size: e.size > -1 ? e.size % MAX_UINT32 : 0
      };
    }
    function toHex(buffer) {
      let hex = "";
      for (const byte of new Uint8Array(buffer)) {
        if (byte < 16) hex += "0";
        hex += byte.toString(16);
      }
      return hex;
    }
    var supportsSubtleSHA1 = null;
    async function shasum(buffer) {
      if (supportsSubtleSHA1 === null) {
        supportsSubtleSHA1 = await testSubtleSHA1();
      }
      return supportsSubtleSHA1 ? subtleSHA1(buffer) : shasumSync(buffer);
    }
    function shasumSync(buffer) {
      return new Hash().update(buffer).digest("hex");
    }
    async function subtleSHA1(buffer) {
      const hash = await crypto.subtle.digest("SHA-1", buffer);
      return toHex(hash);
    }
    async function testSubtleSHA1() {
      try {
        const hash = await subtleSHA1(new Uint8Array([]));
        return hash === "da39a3ee5e6b4b0d3255bfef95601890afd80709";
      } catch (_) {
      }
      return false;
    }
    function parseCacheEntryFlags(bits) {
      return {
        assumeValid: Boolean(bits & 32768),
        extended: Boolean(bits & 16384),
        stage: (bits & 12288) >> 12,
        nameLength: bits & 4095
      };
    }
    function renderCacheEntryFlags(entry) {
      const flags = entry.flags;
      flags.extended = false;
      flags.nameLength = Math.min(Buffer.from(entry.path).length, 4095);
      return (flags.assumeValid ? 32768 : 0) + (flags.extended ? 16384 : 0) + ((flags.stage & 3) << 12) + (flags.nameLength & 4095);
    }
    var GitIndex = class _GitIndex {
      /*::
       _entries: Map<string, CacheEntry>
       _dirty: boolean // Used to determine if index needs to be saved to filesystem
       */
      constructor(entries, unmergedPaths) {
        this._dirty = false;
        this._unmergedPaths = unmergedPaths || /* @__PURE__ */ new Set();
        this._entries = entries || /* @__PURE__ */ new Map();
      }
      _addEntry(entry) {
        if (entry.flags.stage === 0) {
          entry.stages = [entry];
          this._entries.set(entry.path, entry);
          this._unmergedPaths.delete(entry.path);
        } else {
          let existingEntry = this._entries.get(entry.path);
          if (!existingEntry) {
            this._entries.set(entry.path, entry);
            existingEntry = entry;
          }
          existingEntry.stages[entry.flags.stage] = entry;
          this._unmergedPaths.add(entry.path);
        }
      }
      static async from(buffer) {
        if (Buffer.isBuffer(buffer)) {
          return _GitIndex.fromBuffer(buffer);
        } else if (buffer === null) {
          return new _GitIndex(null);
        } else {
          throw new InternalError("invalid type passed to GitIndex.from");
        }
      }
      static async fromBuffer(buffer) {
        if (buffer.length === 0) {
          throw new InternalError("Index file is empty (.git/index)");
        }
        const index2 = new _GitIndex();
        const reader = new BufferCursor(buffer);
        const magic = reader.toString("utf8", 4);
        if (magic !== "DIRC") {
          throw new InternalError(`Invalid dircache magic file number: ${magic}`);
        }
        const shaComputed = await shasum(buffer.slice(0, -20));
        const shaClaimed = buffer.slice(-20).toString("hex");
        if (shaClaimed !== shaComputed) {
          throw new InternalError(
            `Invalid checksum in GitIndex buffer: expected ${shaClaimed} but saw ${shaComputed}`
          );
        }
        const version2 = reader.readUInt32BE();
        if (version2 !== 2) {
          throw new InternalError(`Unsupported dircache version: ${version2}`);
        }
        const numEntries = reader.readUInt32BE();
        let i = 0;
        while (!reader.eof() && i < numEntries) {
          const entry = {};
          entry.ctimeSeconds = reader.readUInt32BE();
          entry.ctimeNanoseconds = reader.readUInt32BE();
          entry.mtimeSeconds = reader.readUInt32BE();
          entry.mtimeNanoseconds = reader.readUInt32BE();
          entry.dev = reader.readUInt32BE();
          entry.ino = reader.readUInt32BE();
          entry.mode = reader.readUInt32BE();
          entry.uid = reader.readUInt32BE();
          entry.gid = reader.readUInt32BE();
          entry.size = reader.readUInt32BE();
          entry.oid = reader.slice(20).toString("hex");
          const flags = reader.readUInt16BE();
          entry.flags = parseCacheEntryFlags(flags);
          const pathlength = buffer.indexOf(0, reader.tell() + 1) - reader.tell();
          if (pathlength < 1) {
            throw new InternalError(`Got a path length of: ${pathlength}`);
          }
          entry.path = reader.toString("utf8", pathlength);
          if (entry.path.includes("..\\") || entry.path.includes("../")) {
            throw new UnsafeFilepathError(entry.path);
          }
          let padding = 8 - (reader.tell() - 12) % 8;
          if (padding === 0) padding = 8;
          while (padding--) {
            const tmp = reader.readUInt8();
            if (tmp !== 0) {
              throw new InternalError(
                `Expected 1-8 null characters but got '${tmp}' after ${entry.path}`
              );
            } else if (reader.eof()) {
              throw new InternalError("Unexpected end of file");
            }
          }
          entry.stages = [];
          index2._addEntry(entry);
          i++;
        }
        return index2;
      }
      get unmergedPaths() {
        return [...this._unmergedPaths];
      }
      get entries() {
        return [...this._entries.values()].sort(comparePath);
      }
      get entriesMap() {
        return this._entries;
      }
      get entriesFlat() {
        return [...this.entries].flatMap((entry) => {
          return entry.stages.length > 1 ? entry.stages.filter((x) => x) : entry;
        });
      }
      *[Symbol.iterator]() {
        for (const entry of this.entries) {
          yield entry;
        }
      }
      insert({ filepath, stats, oid, stage = 0 }) {
        if (!stats) {
          stats = {
            ctimeSeconds: 0,
            ctimeNanoseconds: 0,
            mtimeSeconds: 0,
            mtimeNanoseconds: 0,
            dev: 0,
            ino: 0,
            mode: 0,
            uid: 0,
            gid: 0,
            size: 0
          };
        }
        stats = normalizeStats(stats);
        const bfilepath = Buffer.from(filepath);
        const entry = {
          ctimeSeconds: stats.ctimeSeconds,
          ctimeNanoseconds: stats.ctimeNanoseconds,
          mtimeSeconds: stats.mtimeSeconds,
          mtimeNanoseconds: stats.mtimeNanoseconds,
          dev: stats.dev,
          ino: stats.ino,
          // We provide a fallback value for `mode` here because not all fs
          // implementations assign it, but we use it in GitTree.
          // '100644' is for a "regular non-executable file"
          mode: stats.mode || 33188,
          uid: stats.uid,
          gid: stats.gid,
          size: stats.size,
          path: filepath,
          oid,
          flags: {
            assumeValid: false,
            extended: false,
            stage,
            nameLength: bfilepath.length < 4095 ? bfilepath.length : 4095
          },
          stages: []
        };
        this._addEntry(entry);
        this._dirty = true;
      }
      delete({ filepath }) {
        if (this._entries.has(filepath)) {
          this._entries.delete(filepath);
        } else {
          for (const key of this._entries.keys()) {
            if (key.startsWith(filepath + "/")) {
              this._entries.delete(key);
            }
          }
        }
        if (this._unmergedPaths.has(filepath)) {
          this._unmergedPaths.delete(filepath);
        }
        this._dirty = true;
      }
      clear() {
        this._entries.clear();
        this._dirty = true;
      }
      has({ filepath }) {
        return this._entries.has(filepath);
      }
      render() {
        return this.entries.map((entry) => `${entry.mode.toString(8)} ${entry.oid}    ${entry.path}`).join("\n");
      }
      static async _entryToBuffer(entry) {
        const bpath = Buffer.from(entry.path);
        const length = Math.ceil((62 + bpath.length + 1) / 8) * 8;
        const written = Buffer.alloc(length);
        const writer = new BufferCursor(written);
        const stat = normalizeStats(entry);
        writer.writeUInt32BE(stat.ctimeSeconds);
        writer.writeUInt32BE(stat.ctimeNanoseconds);
        writer.writeUInt32BE(stat.mtimeSeconds);
        writer.writeUInt32BE(stat.mtimeNanoseconds);
        writer.writeUInt32BE(stat.dev);
        writer.writeUInt32BE(stat.ino);
        writer.writeUInt32BE(stat.mode);
        writer.writeUInt32BE(stat.uid);
        writer.writeUInt32BE(stat.gid);
        writer.writeUInt32BE(stat.size);
        writer.write(entry.oid, 20, "hex");
        writer.writeUInt16BE(renderCacheEntryFlags(entry));
        writer.write(entry.path, bpath.length, "utf8");
        return written;
      }
      async toObject() {
        const header = Buffer.alloc(12);
        const writer = new BufferCursor(header);
        writer.write("DIRC", 4, "utf8");
        writer.writeUInt32BE(2);
        writer.writeUInt32BE(this.entriesFlat.length);
        let entryBuffers = [];
        for (const entry of this.entries) {
          entryBuffers.push(_GitIndex._entryToBuffer(entry));
          if (entry.stages.length > 1) {
            for (const stage of entry.stages) {
              if (stage && stage !== entry) {
                entryBuffers.push(_GitIndex._entryToBuffer(stage));
              }
            }
          }
        }
        entryBuffers = await Promise.all(entryBuffers);
        const body = Buffer.concat(entryBuffers);
        const main2 = Buffer.concat([header, body]);
        const sum = await shasum(main2);
        return Buffer.concat([main2, Buffer.from(sum, "hex")]);
      }
    };
    function compareStats(entry, stats, filemode = true, trustino = true) {
      const e = normalizeStats(entry);
      const s = normalizeStats(stats);
      const staleness = filemode && e.mode !== s.mode || e.mtimeSeconds !== s.mtimeSeconds || e.ctimeSeconds !== s.ctimeSeconds || e.uid !== s.uid || e.gid !== s.gid || trustino && e.ino !== s.ino || e.size !== s.size;
      return staleness;
    }
    var lock = null;
    var IndexCache = Symbol("IndexCache");
    function createCache() {
      return {
        map: /* @__PURE__ */ new Map(),
        stats: /* @__PURE__ */ new Map()
      };
    }
    async function updateCachedIndexFile(fs3, filepath, cache) {
      const [stat, rawIndexFile] = await Promise.all([
        fs3.lstat(filepath),
        fs3.read(filepath)
      ]);
      const index2 = await GitIndex.from(rawIndexFile);
      cache.map.set(filepath, index2);
      cache.stats.set(filepath, stat);
    }
    async function isIndexStale(fs3, filepath, cache) {
      const savedStats = cache.stats.get(filepath);
      if (savedStats === void 0) return true;
      if (savedStats === null) return false;
      const currStats = await fs3.lstat(filepath);
      if (currStats === null) return false;
      return compareStats(savedStats, currStats);
    }
    var GitIndexManager = class {
      /**
       * Manages access to the Git index file, ensuring thread-safe operations and caching.
       *
       * @param {object} opts - Options for acquiring the Git index.
       * @param {FSClient} opts.fs - A file system implementation.
       * @param {string} opts.gitdir - The path to the `.git` directory.
       * @param {object} opts.cache - A shared cache object for storing index data.
       * @param {boolean} [opts.allowUnmerged=true] - Whether to allow unmerged paths in the index.
       * @param {function(GitIndex): any} closure - A function to execute with the Git index.
       * @returns {Promise<any>} The result of the closure function.
       * @throws {UnmergedPathsError} If unmerged paths exist and `allowUnmerged` is `false`.
       */
      static async acquire({ fs: fs3, gitdir, cache, allowUnmerged = true }, closure) {
        if (!cache[IndexCache]) {
          cache[IndexCache] = createCache();
        }
        const filepath = `${gitdir}/index`;
        if (lock === null) lock = new AsyncLock({ maxPending: Infinity });
        let result;
        let unmergedPaths = [];
        await lock.acquire(filepath, async () => {
          const theIndexCache = cache[IndexCache];
          if (await isIndexStale(fs3, filepath, theIndexCache)) {
            await updateCachedIndexFile(fs3, filepath, theIndexCache);
          }
          const index2 = theIndexCache.map.get(filepath);
          unmergedPaths = index2.unmergedPaths;
          if (unmergedPaths.length && !allowUnmerged)
            throw new UnmergedPathsError(unmergedPaths);
          result = await closure(index2);
          if (index2._dirty) {
            const buffer = await index2.toObject();
            await fs3.write(filepath, buffer);
            theIndexCache.stats.set(filepath, await fs3.lstat(filepath));
            index2._dirty = false;
          }
        });
        return result;
      }
    };
    function basename(path3) {
      const last = Math.max(path3.lastIndexOf("/"), path3.lastIndexOf("\\"));
      if (last > -1) {
        path3 = path3.slice(last + 1);
      }
      return path3;
    }
    function dirname(path3) {
      const last = Math.max(path3.lastIndexOf("/"), path3.lastIndexOf("\\"));
      if (last === -1) return ".";
      if (last === 0) return "/";
      return path3.slice(0, last);
    }
    function flatFileListToDirectoryStructure(files) {
      const inodes = /* @__PURE__ */ new Map();
      const mkdir = function(name) {
        if (!inodes.has(name)) {
          const dir = {
            type: "tree",
            fullpath: name,
            basename: basename(name),
            metadata: {},
            children: []
          };
          inodes.set(name, dir);
          dir.parent = mkdir(dirname(name));
          if (dir.parent && dir.parent !== dir) dir.parent.children.push(dir);
        }
        return inodes.get(name);
      };
      const mkfile = function(name, metadata) {
        if (!inodes.has(name)) {
          const file = {
            type: "blob",
            fullpath: name,
            basename: basename(name),
            metadata,
            // This recursively generates any missing parent folders.
            parent: mkdir(dirname(name)),
            children: []
          };
          if (file.parent) file.parent.children.push(file);
          inodes.set(name, file);
        }
        return inodes.get(name);
      };
      mkdir(".");
      for (const file of files) {
        mkfile(file.path, file);
      }
      return inodes;
    }
    function mode2type(mode) {
      switch (mode) {
        case 16384:
          return "tree";
        case 33188:
          return "blob";
        case 33261:
          return "blob";
        case 40960:
          return "blob";
        case 57344:
          return "commit";
      }
      throw new InternalError(`Unexpected GitTree entry mode: ${mode.toString(8)}`);
    }
    var GitWalkerIndex = class {
      constructor({ fs: fs3, gitdir, cache }) {
        this.treePromise = GitIndexManager.acquire(
          { fs: fs3, gitdir, cache },
          async function(index2) {
            return flatFileListToDirectoryStructure(index2.entries);
          }
        );
        const walker = this;
        this.ConstructEntry = class StageEntry {
          constructor(fullpath) {
            this._fullpath = fullpath;
            this._type = false;
            this._mode = false;
            this._stat = false;
            this._oid = false;
          }
          async type() {
            return walker.type(this);
          }
          async mode() {
            return walker.mode(this);
          }
          async stat() {
            return walker.stat(this);
          }
          async content() {
            return walker.content(this);
          }
          async oid() {
            return walker.oid(this);
          }
        };
      }
      async readdir(entry) {
        const filepath = entry._fullpath;
        const tree = await this.treePromise;
        const inode = tree.get(filepath);
        if (!inode) return null;
        if (inode.type === "blob") return null;
        if (inode.type !== "tree") {
          throw new Error(`ENOTDIR: not a directory, scandir '${filepath}'`);
        }
        const names = inode.children.map((inode2) => inode2.fullpath);
        names.sort(compareStrings);
        return names;
      }
      async type(entry) {
        if (entry._type === false) {
          await entry.stat();
        }
        return entry._type;
      }
      async mode(entry) {
        if (entry._mode === false) {
          await entry.stat();
        }
        return entry._mode;
      }
      async stat(entry) {
        if (entry._stat === false) {
          const tree = await this.treePromise;
          const inode = tree.get(entry._fullpath);
          if (!inode) {
            throw new Error(
              `ENOENT: no such file or directory, lstat '${entry._fullpath}'`
            );
          }
          const stats = inode.type === "tree" ? {} : normalizeStats(inode.metadata);
          entry._type = inode.type === "tree" ? "tree" : mode2type(stats.mode);
          entry._mode = stats.mode;
          if (inode.type === "tree") {
            entry._stat = void 0;
          } else {
            entry._stat = stats;
          }
        }
        return entry._stat;
      }
      async content(_entry) {
      }
      async oid(entry) {
        if (entry._oid === false) {
          const tree = await this.treePromise;
          const inode = tree.get(entry._fullpath);
          entry._oid = inode.metadata.oid;
        }
        return entry._oid;
      }
    };
    var GitWalkSymbol = Symbol("GitWalkSymbol");
    function STAGE() {
      const o = /* @__PURE__ */ Object.create(null);
      Object.defineProperty(o, GitWalkSymbol, {
        value: function({ fs: fs3, gitdir, cache }) {
          return new GitWalkerIndex({ fs: fs3, gitdir, cache });
        }
      });
      Object.freeze(o);
      return o;
    }
    var NotFoundError = class _NotFoundError extends BaseError {
      /**
       * @param {string} what
       */
      constructor(what) {
        super(`Could not find ${what}.`);
        this.code = this.name = _NotFoundError.code;
        this.data = { what };
      }
    };
    NotFoundError.code = "NotFoundError";
    var ObjectTypeError = class _ObjectTypeError extends BaseError {
      /**
       * @param {string} oid
       * @param {'blob'|'commit'|'tag'|'tree'} actual
       * @param {'blob'|'commit'|'tag'|'tree'} expected
       * @param {string} [filepath]
       */
      constructor(oid, actual, expected, filepath) {
        super(
          `Object ${oid} ${filepath ? `at ${filepath}` : ""}was anticipated to be a ${expected} but it is a ${actual}.`
        );
        this.code = this.name = _ObjectTypeError.code;
        this.data = { oid, actual, expected, filepath };
      }
    };
    ObjectTypeError.code = "ObjectTypeError";
    var InvalidOidError = class _InvalidOidError extends BaseError {
      /**
       * @param {string} value
       */
      constructor(value) {
        super(`Expected a 40-char hex object id but saw "${value}".`);
        this.code = this.name = _InvalidOidError.code;
        this.data = { value };
      }
    };
    InvalidOidError.code = "InvalidOidError";
    var NoRefspecError = class _NoRefspecError extends BaseError {
      /**
       * @param {string} remote
       */
      constructor(remote) {
        super(`Could not find a fetch refspec for remote "${remote}". Make sure the config file has an entry like the following:
[remote "${remote}"]
	fetch = +refs/heads/*:refs/remotes/origin/*
`);
        this.code = this.name = _NoRefspecError.code;
        this.data = { remote };
      }
    };
    NoRefspecError.code = "NoRefspecError";
    var GitPackedRefs = class _GitPackedRefs {
      constructor(text) {
        this.refs = /* @__PURE__ */ new Map();
        this.parsedConfig = [];
        if (text) {
          let key = null;
          this.parsedConfig = text.trim().split("\n").map((line) => {
            if (/^\s*#/.test(line)) {
              return { line, comment: true };
            }
            const i = line.indexOf(" ");
            if (line.startsWith("^")) {
              const value = line.slice(1);
              this.refs.set(key + "^{}", value);
              return { line, ref: key, peeled: value };
            } else {
              const value = line.slice(0, i);
              key = line.slice(i + 1);
              this.refs.set(key, value);
              return { line, ref: key, oid: value };
            }
          });
        }
        return this;
      }
      static from(text) {
        return new _GitPackedRefs(text);
      }
      delete(ref) {
        this.parsedConfig = this.parsedConfig.filter((entry) => entry.ref !== ref);
        this.refs.delete(ref);
      }
      toString() {
        return this.parsedConfig.map(({ line }) => line).join("\n") + "\n";
      }
    };
    var GitRefSpec = class _GitRefSpec {
      constructor({ remotePath, localPath, force, matchPrefix }) {
        Object.assign(this, {
          remotePath,
          localPath,
          force,
          matchPrefix
        });
      }
      static from(refspec) {
        const [forceMatch, remotePath, remoteGlobMatch, localPath, localGlobMatch] = refspec.match(/^(\+?)(.*?)(\*?):(.*?)(\*?)$/).slice(1);
        const force = forceMatch === "+";
        const remoteIsGlob = remoteGlobMatch === "*";
        const localIsGlob = localGlobMatch === "*";
        if (remoteIsGlob !== localIsGlob) {
          throw new InternalError("Invalid refspec");
        }
        return new _GitRefSpec({
          remotePath,
          localPath,
          force,
          matchPrefix: remoteIsGlob
        });
      }
      translate(remoteBranch) {
        if (this.matchPrefix) {
          if (remoteBranch.startsWith(this.remotePath)) {
            return this.localPath + remoteBranch.replace(this.remotePath, "");
          }
        } else {
          if (remoteBranch === this.remotePath) return this.localPath;
        }
        return null;
      }
      reverseTranslate(localBranch) {
        if (this.matchPrefix) {
          if (localBranch.startsWith(this.localPath)) {
            return this.remotePath + localBranch.replace(this.localPath, "");
          }
        } else {
          if (localBranch === this.localPath) return this.remotePath;
        }
        return null;
      }
    };
    var GitRefSpecSet = class _GitRefSpecSet {
      constructor(rules = []) {
        this.rules = rules;
      }
      static from(refspecs) {
        const rules = [];
        for (const refspec of refspecs) {
          rules.push(GitRefSpec.from(refspec));
        }
        return new _GitRefSpecSet(rules);
      }
      add(refspec) {
        const rule = GitRefSpec.from(refspec);
        this.rules.push(rule);
      }
      translate(remoteRefs) {
        const result = [];
        for (const rule of this.rules) {
          for (const remoteRef of remoteRefs) {
            const localRef = rule.translate(remoteRef);
            if (localRef) {
              result.push([remoteRef, localRef]);
            }
          }
        }
        return result;
      }
      translateOne(remoteRef) {
        let result = null;
        for (const rule of this.rules) {
          const localRef = rule.translate(remoteRef);
          if (localRef) {
            result = localRef;
          }
        }
        return result;
      }
      localNamespaces() {
        return this.rules.filter((rule) => rule.matchPrefix).map((rule) => rule.localPath.replace(/\/$/, ""));
      }
    };
    function compareRefNames(a, b) {
      const _a = a.replace(/\^\{\}$/, "");
      const _b = b.replace(/\^\{\}$/, "");
      const tmp = -(_a < _b) || +(_a > _b);
      if (tmp === 0) {
        return a.endsWith("^{}") ? 1 : -1;
      }
      return tmp;
    }
    function normalizeString(path3, aar) {
      let res = "";
      let lastSegmentLength = 0;
      let lastSlash = -1;
      let dots = 0;
      let char = "\0";
      for (let i = 0; i <= path3.length; ++i) {
        if (i < path3.length) char = path3[i];
        else if (char === "/") break;
        else char = "/";
        if (char === "/") {
          if (lastSlash === i - 1 || dots === 1) {
          } else if (dots === 2) {
            if (res.length < 2 || lastSegmentLength !== 2 || res.at(-1) !== "." || res.at(-2) !== ".") {
              if (res.length > 2) {
                const lastSlashIndex = res.lastIndexOf("/");
                if (lastSlashIndex === -1) {
                  res = "";
                  lastSegmentLength = 0;
                } else {
                  res = res.slice(0, lastSlashIndex);
                  lastSegmentLength = res.length - 1 - res.lastIndexOf("/");
                }
                lastSlash = i;
                dots = 0;
                continue;
              } else if (res.length !== 0) {
                res = "";
                lastSegmentLength = 0;
                lastSlash = i;
                dots = 0;
                continue;
              }
            }
            if (aar) {
              res += res.length > 0 ? "/.." : "..";
              lastSegmentLength = 2;
            }
          } else {
            if (res.length > 0) res += "/" + path3.slice(lastSlash + 1, i);
            else res = path3.slice(lastSlash + 1, i);
            lastSegmentLength = i - lastSlash - 1;
          }
          lastSlash = i;
          dots = 0;
        } else if (char === "." && dots !== -1) {
          ++dots;
        } else {
          dots = -1;
        }
      }
      return res;
    }
    function normalize(path3) {
      if (!path3.length) return ".";
      const isAbsolute2 = path3[0] === "/";
      const trailingSeparator = path3.at(-1) === "/";
      path3 = normalizeString(path3, !isAbsolute2);
      if (!path3.length) {
        if (isAbsolute2) return "/";
        return trailingSeparator ? "./" : ".";
      }
      if (trailingSeparator) path3 += "/";
      return isAbsolute2 ? `/${path3}` : path3;
    }
    function join2(...args) {
      if (args.length === 0) return ".";
      let joined;
      for (let i = 0; i < args.length; ++i) {
        const arg = args[i];
        if (arg.length > 0) {
          if (joined === void 0) joined = arg;
          else joined += "/" + arg;
        }
      }
      if (joined === void 0) return ".";
      return normalize(joined);
    }
    var num = (val) => {
      if (typeof val === "number") {
        return val;
      }
      val = val.toLowerCase();
      let n = parseInt(val);
      if (val.endsWith("k")) n *= 1024;
      if (val.endsWith("m")) n *= 1024 * 1024;
      if (val.endsWith("g")) n *= 1024 * 1024 * 1024;
      return n;
    };
    var bool = (val) => {
      if (typeof val === "boolean") {
        return val;
      }
      val = val.trim().toLowerCase();
      if (val === "true" || val === "yes" || val === "on") return true;
      if (val === "false" || val === "no" || val === "off") return false;
      throw Error(
        `Expected 'true', 'false', 'yes', 'no', 'on', or 'off', but got ${val}`
      );
    };
    var schema = {
      core: {
        filemode: bool,
        bare: bool,
        logallrefupdates: bool,
        symlinks: bool,
        ignorecase: bool,
        bigFileThreshold: num
      }
    };
    var SECTION_LINE_REGEX = /^\[([A-Za-z0-9-.]+)(?: "(.*)")?\]$/;
    var SECTION_REGEX = /^[A-Za-z0-9-.]+$/;
    var VARIABLE_LINE_REGEX = /^([A-Za-z][A-Za-z-]*)(?: *= *(.*))?$/;
    var VARIABLE_NAME_REGEX = /^[A-Za-z][A-Za-z-]*$/;
    var VARIABLE_VALUE_COMMENT_REGEX = /^(.*?)( *[#;].*)$/;
    var extractSectionLine = (line) => {
      const matches = SECTION_LINE_REGEX.exec(line);
      if (matches != null) {
        const [section, subsection] = matches.slice(1);
        return [section, subsection];
      }
      return null;
    };
    var extractVariableLine = (line) => {
      const matches = VARIABLE_LINE_REGEX.exec(line);
      if (matches != null) {
        const [name, rawValue = "true"] = matches.slice(1);
        const valueWithoutComments = removeComments(rawValue);
        const valueWithoutQuotes = removeQuotes(valueWithoutComments);
        return [name, valueWithoutQuotes];
      }
      return null;
    };
    var removeComments = (rawValue) => {
      const commentMatches = VARIABLE_VALUE_COMMENT_REGEX.exec(rawValue);
      if (commentMatches == null) {
        return rawValue;
      }
      const [valueWithoutComment, comment] = commentMatches.slice(1);
      if (hasOddNumberOfQuotes(valueWithoutComment) && hasOddNumberOfQuotes(comment)) {
        return `${valueWithoutComment}${comment}`;
      }
      return valueWithoutComment;
    };
    var hasOddNumberOfQuotes = (text) => {
      const numberOfQuotes = (text.match(/(?:^|[^\\])"/g) || []).length;
      return numberOfQuotes % 2 !== 0;
    };
    var removeQuotes = (text) => {
      return text.split("").reduce((newText, c, idx, text2) => {
        const isQuote = c === '"' && text2[idx - 1] !== "\\";
        const isEscapeForQuote = c === "\\" && text2[idx + 1] === '"';
        if (isQuote || isEscapeForQuote) {
          return newText;
        }
        return newText + c;
      }, "");
    };
    var lower = (text) => {
      return text != null ? text.toLowerCase() : null;
    };
    var getPath = (section, subsection, name) => {
      return [lower(section), subsection, lower(name)].filter((a) => a != null).join(".");
    };
    var normalizePath = (path3) => {
      const pathSegments = path3.split(".");
      const section = pathSegments.shift();
      const name = pathSegments.pop();
      const subsection = pathSegments.length ? pathSegments.join(".") : void 0;
      return {
        section,
        subsection,
        name,
        path: getPath(section, subsection, name),
        sectionPath: getPath(section, subsection, null),
        isSection: !!section
      };
    };
    var findLastIndex = (array, callback) => {
      return array.reduce((lastIndex, item, index2) => {
        return callback(item) ? index2 : lastIndex;
      }, -1);
    };
    var GitConfig = class _GitConfig {
      constructor(text) {
        let section = null;
        let subsection = null;
        this.parsedConfig = text ? text.split("\n").map((line) => {
          let name = null;
          let value = null;
          const trimmedLine = line.trim();
          const extractedSection = extractSectionLine(trimmedLine);
          const isSection = extractedSection != null;
          if (isSection) {
            ;
            [section, subsection] = extractedSection;
          } else {
            const extractedVariable = extractVariableLine(trimmedLine);
            const isVariable = extractedVariable != null;
            if (isVariable) {
              ;
              [name, value] = extractedVariable;
            }
          }
          const path3 = getPath(section, subsection, name);
          return { line, isSection, section, subsection, name, value, path: path3 };
        }) : [];
      }
      static from(text) {
        return new _GitConfig(text);
      }
      async get(path3, getall = false) {
        const normalizedPath = normalizePath(path3).path;
        const allValues = this.parsedConfig.filter((config) => config.path === normalizedPath).map(({ section, name, value }) => {
          const fn = schema[section] && schema[section][name];
          return fn ? fn(value) : value;
        });
        return getall ? allValues : allValues.pop();
      }
      async getall(path3) {
        return this.get(path3, true);
      }
      async getSubsections(section) {
        return this.parsedConfig.filter((config) => config.isSection && config.section === section).map((config) => config.subsection);
      }
      async deleteSection(section, subsection) {
        this.parsedConfig = this.parsedConfig.filter(
          (config) => !(config.section === section && config.subsection === subsection)
        );
      }
      async append(path3, value) {
        return this.set(path3, value, true);
      }
      async set(path3, value, append = false) {
        const {
          section,
          subsection,
          name,
          path: normalizedPath,
          sectionPath,
          isSection
        } = normalizePath(path3);
        const configIndex = findLastIndex(
          this.parsedConfig,
          (config) => config.path === normalizedPath
        );
        if (value == null) {
          if (configIndex !== -1) {
            this.parsedConfig.splice(configIndex, 1);
          }
        } else {
          if (configIndex !== -1) {
            const config = this.parsedConfig[configIndex];
            const modifiedConfig = Object.assign({}, config, {
              name,
              value,
              modified: true
            });
            if (append) {
              this.parsedConfig.splice(configIndex + 1, 0, modifiedConfig);
            } else {
              this.parsedConfig[configIndex] = modifiedConfig;
            }
          } else {
            const sectionIndex = this.parsedConfig.findIndex(
              (config) => config.path === sectionPath
            );
            const newConfig = {
              section,
              subsection,
              name,
              value,
              modified: true,
              path: normalizedPath
            };
            if (SECTION_REGEX.test(section) && VARIABLE_NAME_REGEX.test(name)) {
              if (sectionIndex >= 0) {
                this.parsedConfig.splice(sectionIndex + 1, 0, newConfig);
              } else {
                const newSection = {
                  isSection,
                  section,
                  subsection,
                  modified: true,
                  path: sectionPath
                };
                this.parsedConfig.push(newSection, newConfig);
              }
            }
          }
        }
      }
      toString() {
        return this.parsedConfig.map(({ line, section, subsection, name, value, modified: modified2 = false }) => {
          if (!modified2) {
            return line;
          }
          if (name != null && value != null) {
            if (typeof value === "string" && /[#;]/.test(value)) {
              return `	${name} = "${value}"`;
            }
            return `	${name} = ${value}`;
          }
          if (subsection != null) {
            return `[${section} "${subsection}"]`;
          }
          return `[${section}]`;
        }).join("\n");
      }
    };
    var GitConfigManager = class {
      /**
       * Reads the Git configuration file from the specified `.git` directory.
       *
       * @param {object} opts - Options for reading the Git configuration.
       * @param {FSClient} opts.fs - A file system implementation.
       * @param {string} opts.gitdir - The path to the `.git` directory.
       * @returns {Promise<GitConfig>} A `GitConfig` object representing the parsed configuration.
       */
      static async get({ fs: fs3, gitdir }) {
        const text = await fs3.read(`${gitdir}/config`, { encoding: "utf8" });
        return GitConfig.from(text);
      }
      /**
       * Saves the provided Git configuration to the specified `.git` directory.
       *
       * @param {object} opts - Options for saving the Git configuration.
       * @param {FSClient} opts.fs - A file system implementation.
       * @param {string} opts.gitdir - The path to the `.git` directory.
       * @param {GitConfig} opts.config - The `GitConfig` object to save.
       * @returns {Promise<void>} Resolves when the configuration has been successfully saved.
       */
      static async save({ fs: fs3, gitdir, config }) {
        await fs3.write(`${gitdir}/config`, config.toString(), {
          encoding: "utf8"
        });
      }
    };
    var refpaths = (ref) => [
      `${ref}`,
      `refs/${ref}`,
      `refs/tags/${ref}`,
      `refs/heads/${ref}`,
      `refs/remotes/${ref}`,
      `refs/remotes/${ref}/HEAD`
    ];
    var GIT_FILES = ["config", "description", "index", "shallow", "commondir"];
    var lock$1;
    async function acquireLock(ref, callback) {
      if (lock$1 === void 0) lock$1 = new AsyncLock();
      return lock$1.acquire(ref, callback);
    }
    var GitRefManager = class _GitRefManager {
      /**
       * Updates remote refs based on the provided refspecs and options.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir=join(dir, '.git')] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} args.remote - The name of the remote.
       * @param {Map<string, string>} args.refs - A map of refs to their object IDs.
       * @param {Map<string, string>} args.symrefs - A map of symbolic refs.
       * @param {boolean} args.tags - Whether to fetch tags.
       * @param {string[]} [args.refspecs = undefined] - The refspecs to use.
       * @param {boolean} [args.prune = false] - Whether to prune stale refs.
       * @param {boolean} [args.pruneTags = false] - Whether to prune tags.
       * @returns {Promise<Object>} - An object containing pruned refs.
       */
      static async updateRemoteRefs({
        fs: fs3,
        gitdir,
        remote,
        refs,
        symrefs,
        tags,
        refspecs = void 0,
        prune = false,
        pruneTags = false
      }) {
        for (const value of refs.values()) {
          if (!value.match(/[0-9a-f]{40}/)) {
            throw new InvalidOidError(value);
          }
        }
        const config = await GitConfigManager.get({ fs: fs3, gitdir });
        if (!refspecs) {
          refspecs = await config.getall(`remote.${remote}.fetch`);
          if (refspecs.length === 0) {
            throw new NoRefspecError(remote);
          }
          refspecs.unshift(`+HEAD:refs/remotes/${remote}/HEAD`);
        }
        const refspec = GitRefSpecSet.from(refspecs);
        const actualRefsToWrite = /* @__PURE__ */ new Map();
        if (pruneTags) {
          const tags2 = await _GitRefManager.listRefs({
            fs: fs3,
            gitdir,
            filepath: "refs/tags"
          });
          await _GitRefManager.deleteRefs({
            fs: fs3,
            gitdir,
            refs: tags2.map((tag2) => `refs/tags/${tag2}`)
          });
        }
        if (tags) {
          for (const serverRef of refs.keys()) {
            if (serverRef.startsWith("refs/tags") && !serverRef.endsWith("^{}")) {
              if (!await _GitRefManager.exists({ fs: fs3, gitdir, ref: serverRef })) {
                const oid = refs.get(serverRef);
                actualRefsToWrite.set(serverRef, oid);
              }
            }
          }
        }
        const refTranslations = refspec.translate([...refs.keys()]);
        for (const [serverRef, translatedRef] of refTranslations) {
          const value = refs.get(serverRef);
          actualRefsToWrite.set(translatedRef, value);
        }
        const symrefTranslations = refspec.translate([...symrefs.keys()]);
        for (const [serverRef, translatedRef] of symrefTranslations) {
          const value = symrefs.get(serverRef);
          const symtarget = refspec.translateOne(value);
          if (symtarget) {
            actualRefsToWrite.set(translatedRef, `ref: ${symtarget}`);
          }
        }
        const pruned = [];
        if (prune) {
          for (const filepath of refspec.localNamespaces()) {
            const refs2 = (await _GitRefManager.listRefs({
              fs: fs3,
              gitdir,
              filepath
            })).map((file) => `${filepath}/${file}`);
            for (const ref of refs2) {
              if (!actualRefsToWrite.has(ref)) {
                pruned.push(ref);
              }
            }
          }
          if (pruned.length > 0) {
            await _GitRefManager.deleteRefs({ fs: fs3, gitdir, refs: pruned });
          }
        }
        for (const [key, value] of actualRefsToWrite) {
          await acquireLock(
            key,
            async () => fs3.write(join2(gitdir, key), `${value.trim()}
`, "utf8")
          );
        }
        return { pruned };
      }
      /**
       * Writes a ref to the file system.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} args.ref - The ref to write.
       * @param {string} args.value - The object ID to write.
       * @returns {Promise<void>}
       */
      // TODO: make this less crude?
      static async writeRef({ fs: fs3, gitdir, ref, value }) {
        if (!value.match(/[0-9a-f]{40}/)) {
          throw new InvalidOidError(value);
        }
        await acquireLock(
          ref,
          async () => fs3.write(join2(gitdir, ref), `${value.trim()}
`, "utf8")
        );
      }
      /**
       * Writes a symbolic ref to the file system.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} args.ref - The ref to write.
       * @param {string} args.value - The target ref.
       * @returns {Promise<void>}
       */
      static async writeSymbolicRef({ fs: fs3, gitdir, ref, value }) {
        await acquireLock(
          ref,
          async () => fs3.write(join2(gitdir, ref), `ref: ${value.trim()}
`, "utf8")
        );
      }
      /**
       * Deletes a single ref.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} args.ref - The ref to delete.
       * @returns {Promise<void>}
       */
      static async deleteRef({ fs: fs3, gitdir, ref }) {
        return _GitRefManager.deleteRefs({ fs: fs3, gitdir, refs: [ref] });
      }
      /**
       * Deletes multiple refs.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string[]} args.refs - The refs to delete.
       * @returns {Promise<void>}
       */
      static async deleteRefs({ fs: fs3, gitdir, refs }) {
        await Promise.all(refs.map((ref) => fs3.rm(join2(gitdir, ref))));
        let text = await acquireLock(
          "packed-refs",
          async () => fs3.read(`${gitdir}/packed-refs`, { encoding: "utf8" })
        );
        const packed = GitPackedRefs.from(text);
        const beforeSize = packed.refs.size;
        for (const ref of refs) {
          if (packed.refs.has(ref)) {
            packed.delete(ref);
          }
        }
        if (packed.refs.size < beforeSize) {
          text = packed.toString();
          await acquireLock(
            "packed-refs",
            async () => fs3.write(`${gitdir}/packed-refs`, text, { encoding: "utf8" })
          );
        }
      }
      /**
       * Resolves a ref to its object ID.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} args.ref - The ref to resolve.
       * @param {number} [args.depth = undefined] - The maximum depth to resolve symbolic refs.
       * @returns {Promise<string>} - The resolved object ID.
       */
      static async resolve({ fs: fs3, gitdir, ref, depth = void 0 }) {
        if (depth !== void 0) {
          depth--;
          if (depth === -1) {
            return ref;
          }
        }
        if (ref.startsWith("ref: ")) {
          ref = ref.slice("ref: ".length);
          return _GitRefManager.resolve({ fs: fs3, gitdir, ref, depth });
        }
        if (ref.length === 40 && /[0-9a-f]{40}/.test(ref)) {
          return ref;
        }
        const packedMap = await _GitRefManager.packedRefs({ fs: fs3, gitdir });
        const allpaths = refpaths(ref).filter((p) => !GIT_FILES.includes(p));
        for (const ref2 of allpaths) {
          const sha = await acquireLock(
            ref2,
            async () => await fs3.read(`${gitdir}/${ref2}`, { encoding: "utf8" }) || packedMap.get(ref2)
          );
          if (sha) {
            return _GitRefManager.resolve({ fs: fs3, gitdir, ref: sha.trim(), depth });
          }
        }
        throw new NotFoundError(ref);
      }
      /**
       * Checks if a ref exists.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir=join(dir, '.git')] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} args.ref - The ref to check.
       * @returns {Promise<boolean>} - True if the ref exists, false otherwise.
       */
      static async exists({ fs: fs3, gitdir, ref }) {
        try {
          await _GitRefManager.expand({ fs: fs3, gitdir, ref });
          return true;
        } catch (err) {
          return false;
        }
      }
      /**
       * Expands a ref to its full name.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir=join(dir, '.git')] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} args.ref - The ref to expand.
       * @returns {Promise<string>} - The full ref name.
       */
      static async expand({ fs: fs3, gitdir, ref }) {
        if (ref.length === 40 && /[0-9a-f]{40}/.test(ref)) {
          return ref;
        }
        const packedMap = await _GitRefManager.packedRefs({ fs: fs3, gitdir });
        const allpaths = refpaths(ref);
        for (const ref2 of allpaths) {
          const refExists = await acquireLock(
            ref2,
            async () => fs3.exists(`${gitdir}/${ref2}`)
          );
          if (refExists) return ref2;
          if (packedMap.has(ref2)) return ref2;
        }
        throw new NotFoundError(ref);
      }
      /**
       * Expands a ref against a provided map.
       *
       * @param {Object} args
       * @param {string} args.ref - The ref to expand.
       * @param {Map<string, string>} args.map - The map of refs.
       * @returns {Promise<string>} - The expanded ref.
       */
      static async expandAgainstMap({ ref, map }) {
        const allpaths = refpaths(ref);
        for (const ref2 of allpaths) {
          if (await map.has(ref2)) return ref2;
        }
        throw new NotFoundError(ref);
      }
      /**
       * Resolves a ref against a provided map.
       *
       * @param {Object} args
       * @param {string} args.ref - The ref to resolve.
       * @param {string} [args.fullref = args.ref] - The full ref name.
       * @param {number} [args.depth = undefined] - The maximum depth to resolve symbolic refs.
       * @param {Map<string, string>} args.map - The map of refs.
       * @returns {Object} - An object containing the full ref and its object ID.
       */
      static resolveAgainstMap({ ref, fullref = ref, depth = void 0, map }) {
        if (depth !== void 0) {
          depth--;
          if (depth === -1) {
            return { fullref, oid: ref };
          }
        }
        if (ref.startsWith("ref: ")) {
          ref = ref.slice("ref: ".length);
          return _GitRefManager.resolveAgainstMap({ ref, fullref, depth, map });
        }
        if (ref.length === 40 && /[0-9a-f]{40}/.test(ref)) {
          return { fullref, oid: ref };
        }
        const allpaths = refpaths(ref);
        for (const ref2 of allpaths) {
          const sha = map.get(ref2);
          if (sha) {
            return _GitRefManager.resolveAgainstMap({
              ref: sha.trim(),
              fullref: ref2,
              depth,
              map
            });
          }
        }
        throw new NotFoundError(ref);
      }
      /**
       * Reads the packed refs file and returns a map of refs.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir=join(dir, '.git')] - [required] The [git directory](dir-vs-gitdir.md) path
       * @returns {Promise<Map<string, string>>} - A map of packed refs.
       */
      static async packedRefs({ fs: fs3, gitdir }) {
        const text = await acquireLock(
          "packed-refs",
          async () => fs3.read(`${gitdir}/packed-refs`, { encoding: "utf8" })
        );
        const packed = GitPackedRefs.from(text);
        return packed.refs;
      }
      /**
       * Lists all refs matching a given filepath prefix.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir=join(dir, '.git')] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} args.filepath - The filepath prefix to match.
       * @returns {Promise<string[]>} - A sorted list of refs.
       */
      static async listRefs({ fs: fs3, gitdir, filepath }) {
        const packedMap = _GitRefManager.packedRefs({ fs: fs3, gitdir });
        let files = null;
        try {
          files = await fs3.readdirDeep(`${gitdir}/${filepath}`);
          files = files.map((x) => x.replace(`${gitdir}/${filepath}/`, ""));
        } catch (err) {
          files = [];
        }
        for (let key of (await packedMap).keys()) {
          if (key.startsWith(filepath)) {
            key = key.replace(filepath + "/", "");
            if (!files.includes(key)) {
              files.push(key);
            }
          }
        }
        files.sort(compareRefNames);
        return files;
      }
      /**
       * Lists all branches, optionally filtered by remote.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir=join(dir, '.git')] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} [args.remote] - The remote to filter branches by.
       * @returns {Promise<string[]>} - A list of branch names.
       */
      static async listBranches({ fs: fs3, gitdir, remote }) {
        if (remote) {
          return _GitRefManager.listRefs({
            fs: fs3,
            gitdir,
            filepath: `refs/remotes/${remote}`
          });
        } else {
          return _GitRefManager.listRefs({ fs: fs3, gitdir, filepath: `refs/heads` });
        }
      }
      /**
       * Lists all tags.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir=join(dir, '.git')] - [required] The [git directory](dir-vs-gitdir.md) path
       * @returns {Promise<string[]>} - A list of tag names.
       */
      static async listTags({ fs: fs3, gitdir }) {
        const tags = await _GitRefManager.listRefs({
          fs: fs3,
          gitdir,
          filepath: `refs/tags`
        });
        return tags.filter((x) => !x.endsWith("^{}"));
      }
    };
    function compareTreeEntryPath(a, b) {
      return compareStrings(appendSlashIfDir(a), appendSlashIfDir(b));
    }
    function appendSlashIfDir(entry) {
      return entry.mode === "040000" ? entry.path + "/" : entry.path;
    }
    function mode2type$1(mode) {
      switch (mode) {
        case "040000":
          return "tree";
        case "100644":
          return "blob";
        case "100755":
          return "blob";
        case "120000":
          return "blob";
        case "160000":
          return "commit";
      }
      throw new InternalError(`Unexpected GitTree entry mode: ${mode}`);
    }
    function parseBuffer(buffer) {
      const _entries = [];
      let cursor = 0;
      while (cursor < buffer.length) {
        const space = buffer.indexOf(32, cursor);
        if (space === -1) {
          throw new InternalError(
            `GitTree: Error parsing buffer at byte location ${cursor}: Could not find the next space character.`
          );
        }
        const nullchar = buffer.indexOf(0, cursor);
        if (nullchar === -1) {
          throw new InternalError(
            `GitTree: Error parsing buffer at byte location ${cursor}: Could not find the next null character.`
          );
        }
        let mode = buffer.slice(cursor, space).toString("utf8");
        if (mode === "40000") mode = "040000";
        const type = mode2type$1(mode);
        const path3 = buffer.slice(space + 1, nullchar).toString("utf8");
        if (path3.includes("\\") || path3.includes("/")) {
          throw new UnsafeFilepathError(path3);
        }
        const oid = buffer.slice(nullchar + 1, nullchar + 21).toString("hex");
        cursor = nullchar + 21;
        _entries.push({ mode, path: path3, oid, type });
      }
      return _entries;
    }
    function limitModeToAllowed(mode) {
      if (typeof mode === "number") {
        mode = mode.toString(8);
      }
      if (mode.match(/^0?4.*/)) return "040000";
      if (mode.match(/^1006.*/)) return "100644";
      if (mode.match(/^1007.*/)) return "100755";
      if (mode.match(/^120.*/)) return "120000";
      if (mode.match(/^160.*/)) return "160000";
      throw new InternalError(`Could not understand file mode: ${mode}`);
    }
    function nudgeIntoShape(entry) {
      if (!entry.oid && entry.sha) {
        entry.oid = entry.sha;
      }
      entry.mode = limitModeToAllowed(entry.mode);
      if (!entry.type) {
        entry.type = mode2type$1(entry.mode);
      }
      return entry;
    }
    var GitTree = class _GitTree {
      constructor(entries) {
        if (Buffer.isBuffer(entries)) {
          this._entries = parseBuffer(entries);
        } else if (Array.isArray(entries)) {
          this._entries = entries.map(nudgeIntoShape);
        } else {
          throw new InternalError("invalid type passed to GitTree constructor");
        }
        this._entries.sort(comparePath);
      }
      static from(tree) {
        return new _GitTree(tree);
      }
      render() {
        return this._entries.map((entry) => `${entry.mode} ${entry.type} ${entry.oid}    ${entry.path}`).join("\n");
      }
      toObject() {
        const entries = [...this._entries];
        entries.sort(compareTreeEntryPath);
        return Buffer.concat(
          entries.map((entry) => {
            const mode = Buffer.from(entry.mode.replace(/^0/, ""));
            const space = Buffer.from(" ");
            const path3 = Buffer.from(entry.path, "utf8");
            const nullchar = Buffer.from([0]);
            const oid = Buffer.from(entry.oid, "hex");
            return Buffer.concat([mode, space, path3, nullchar, oid]);
          })
        );
      }
      /**
       * @returns {TreeEntry[]}
       */
      entries() {
        return this._entries;
      }
      *[Symbol.iterator]() {
        for (const entry of this._entries) {
          yield entry;
        }
      }
    };
    var GitObject = class {
      /**
       * Wraps a raw object with a Git header.
       *
       * @param {Object} params - The parameters for wrapping.
       * @param {string} params.type - The type of the Git object (e.g., 'blob', 'tree', 'commit').
       * @param {Uint8Array} params.object - The raw object data to wrap.
       * @returns {Uint8Array} The wrapped Git object as a single buffer.
       */
      static wrap({ type, object }) {
        const header = `${type} ${object.length}\0`;
        const headerLen = header.length;
        const totalLength = headerLen + object.length;
        const wrappedObject = new Uint8Array(totalLength);
        for (let i = 0; i < headerLen; i++) {
          wrappedObject[i] = header.charCodeAt(i);
        }
        wrappedObject.set(object, headerLen);
        return wrappedObject;
      }
      /**
       * Unwraps a Git object buffer into its type and raw object data.
       *
       * @param {Buffer|Uint8Array} buffer - The buffer containing the wrapped Git object.
       * @returns {{ type: string, object: Buffer }} An object containing the type and the raw object data.
       * @throws {InternalError} If the length specified in the header does not match the actual object length.
       */
      static unwrap(buffer) {
        const s = buffer.indexOf(32);
        const i = buffer.indexOf(0);
        const type = buffer.slice(0, s).toString("utf8");
        const length = buffer.slice(s + 1, i).toString("utf8");
        const actualLength = buffer.length - (i + 1);
        if (parseInt(length) !== actualLength) {
          throw new InternalError(
            `Length mismatch: expected ${length} bytes but got ${actualLength} instead.`
          );
        }
        return {
          type,
          object: Buffer.from(buffer.slice(i + 1))
        };
      }
    };
    async function readObjectLoose({ fs: fs3, gitdir, oid }) {
      const source = `objects/${oid.slice(0, 2)}/${oid.slice(2)}`;
      const file = await fs3.read(`${gitdir}/${source}`);
      if (!file) {
        return null;
      }
      return { object: file, format: "deflated", source };
    }
    function applyDelta(delta, source) {
      const reader = new BufferCursor(delta);
      const sourceSize = readVarIntLE(reader);
      if (sourceSize !== source.byteLength) {
        throw new InternalError(
          `applyDelta expected source buffer to be ${sourceSize} bytes but the provided buffer was ${source.length} bytes`
        );
      }
      const targetSize = readVarIntLE(reader);
      let target;
      const firstOp = readOp(reader, source);
      if (firstOp.byteLength === targetSize) {
        target = firstOp;
      } else {
        target = Buffer.alloc(targetSize);
        const writer = new BufferCursor(target);
        writer.copy(firstOp);
        while (!reader.eof()) {
          writer.copy(readOp(reader, source));
        }
        const tell = writer.tell();
        if (targetSize !== tell) {
          throw new InternalError(
            `applyDelta expected target buffer to be ${targetSize} bytes but the resulting buffer was ${tell} bytes`
          );
        }
      }
      return target;
    }
    function readVarIntLE(reader) {
      let result = 0;
      let shift = 0;
      let byte = null;
      do {
        byte = reader.readUInt8();
        result |= (byte & 127) << shift;
        shift += 7;
      } while (byte & 128);
      return result;
    }
    function readCompactLE(reader, flags, size) {
      let result = 0;
      let shift = 0;
      while (size--) {
        if (flags & 1) {
          result |= reader.readUInt8() << shift;
        }
        flags >>= 1;
        shift += 8;
      }
      return result;
    }
    function readOp(reader, source) {
      const byte = reader.readUInt8();
      const COPY = 128;
      const OFFS = 15;
      const SIZE = 112;
      if (byte & COPY) {
        const offset = readCompactLE(reader, byte & OFFS, 4);
        let size = readCompactLE(reader, (byte & SIZE) >> 4, 3);
        if (size === 0) size = 65536;
        return source.slice(offset, offset + size);
      } else {
        return reader.slice(byte);
      }
    }
    function fromValue(value) {
      let queue = [value];
      return {
        next() {
          return Promise.resolve({ done: queue.length === 0, value: queue.pop() });
        },
        return() {
          queue = [];
          return {};
        },
        [Symbol.asyncIterator]() {
          return this;
        }
      };
    }
    function getIterator(iterable) {
      if (iterable[Symbol.asyncIterator]) {
        return iterable[Symbol.asyncIterator]();
      }
      if (iterable[Symbol.iterator]) {
        return iterable[Symbol.iterator]();
      }
      if (iterable.next) {
        return iterable;
      }
      return fromValue(iterable);
    }
    var StreamReader = class {
      constructor(stream) {
        if (typeof Buffer === "undefined") {
          throw new Error("Missing Buffer dependency");
        }
        this.stream = getIterator(stream);
        this.buffer = null;
        this.cursor = 0;
        this.undoCursor = 0;
        this.started = false;
        this._ended = false;
        this._discardedBytes = 0;
      }
      eof() {
        return this._ended && this.cursor === this.buffer.length;
      }
      tell() {
        return this._discardedBytes + this.cursor;
      }
      async byte() {
        if (this.eof()) return;
        if (!this.started) await this._init();
        if (this.cursor === this.buffer.length) {
          await this._loadnext();
          if (this._ended) return;
        }
        this._moveCursor(1);
        return this.buffer[this.undoCursor];
      }
      async chunk() {
        if (this.eof()) return;
        if (!this.started) await this._init();
        if (this.cursor === this.buffer.length) {
          await this._loadnext();
          if (this._ended) return;
        }
        this._moveCursor(this.buffer.length);
        return this.buffer.slice(this.undoCursor, this.cursor);
      }
      async read(n) {
        if (this.eof()) return;
        if (!this.started) await this._init();
        if (this.cursor + n > this.buffer.length) {
          this._trim();
          await this._accumulate(n);
        }
        this._moveCursor(n);
        return this.buffer.slice(this.undoCursor, this.cursor);
      }
      async skip(n) {
        if (this.eof()) return;
        if (!this.started) await this._init();
        if (this.cursor + n > this.buffer.length) {
          this._trim();
          await this._accumulate(n);
        }
        this._moveCursor(n);
      }
      async undo() {
        this.cursor = this.undoCursor;
      }
      async _next() {
        this.started = true;
        let { done, value } = await this.stream.next();
        if (done) {
          this._ended = true;
          if (!value) return Buffer.alloc(0);
        }
        if (value) {
          value = Buffer.from(value);
        }
        return value;
      }
      _trim() {
        this.buffer = this.buffer.slice(this.undoCursor);
        this.cursor -= this.undoCursor;
        this._discardedBytes += this.undoCursor;
        this.undoCursor = 0;
      }
      _moveCursor(n) {
        this.undoCursor = this.cursor;
        this.cursor += n;
        if (this.cursor > this.buffer.length) {
          this.cursor = this.buffer.length;
        }
      }
      async _accumulate(n) {
        if (this._ended) return;
        const buffers = [this.buffer];
        while (this.cursor + n > lengthBuffers(buffers)) {
          const nextbuffer = await this._next();
          if (this._ended) break;
          buffers.push(nextbuffer);
        }
        this.buffer = Buffer.concat(buffers);
      }
      async _loadnext() {
        this._discardedBytes += this.buffer.length;
        this.undoCursor = 0;
        this.cursor = 0;
        this.buffer = await this._next();
      }
      async _init() {
        this.buffer = await this._next();
      }
    };
    function lengthBuffers(buffers) {
      return buffers.reduce((acc, buffer) => acc + buffer.length, 0);
    }
    async function listpack(stream, onData) {
      const reader = new StreamReader(stream);
      let PACK = await reader.read(4);
      PACK = PACK.toString("utf8");
      if (PACK !== "PACK") {
        throw new InternalError(`Invalid PACK header '${PACK}'`);
      }
      let version2 = await reader.read(4);
      version2 = version2.readUInt32BE(0);
      if (version2 !== 2) {
        throw new InternalError(`Invalid packfile version: ${version2}`);
      }
      let numObjects = await reader.read(4);
      numObjects = numObjects.readUInt32BE(0);
      if (numObjects < 1) return;
      while (!reader.eof() && numObjects--) {
        const offset = reader.tell();
        const { type, length, ofs, reference } = await parseHeader(reader);
        const inflator = new pako.Inflate();
        while (!inflator.result) {
          const chunk = await reader.chunk();
          if (!chunk) break;
          inflator.push(chunk, false);
          if (inflator.err) {
            throw new InternalError(`Pako error: ${inflator.msg}`);
          }
          if (inflator.result) {
            if (inflator.result.length !== length) {
              throw new InternalError(
                `Inflated object size is different from that stated in packfile.`
              );
            }
            await reader.undo();
            await reader.read(chunk.length - inflator.strm.avail_in);
            const end = reader.tell();
            await onData({
              data: inflator.result,
              type,
              num: numObjects,
              offset,
              end,
              reference,
              ofs
            });
          }
        }
      }
    }
    async function parseHeader(reader) {
      let byte = await reader.byte();
      const type = byte >> 4 & 7;
      let length = byte & 15;
      if (byte & 128) {
        let shift = 4;
        do {
          byte = await reader.byte();
          length |= (byte & 127) << shift;
          shift += 7;
        } while (byte & 128);
      }
      let ofs;
      let reference;
      if (type === 6) {
        let shift = 0;
        ofs = 0;
        const bytes = [];
        do {
          byte = await reader.byte();
          ofs |= (byte & 127) << shift;
          shift += 7;
          bytes.push(byte);
        } while (byte & 128);
        reference = Buffer.from(bytes);
      }
      if (type === 7) {
        const buf = await reader.read(20);
        reference = buf;
      }
      return { type, length, ofs, reference };
    }
    var supportsDecompressionStream = false;
    async function inflate(buffer) {
      if (supportsDecompressionStream === null) {
        supportsDecompressionStream = testDecompressionStream();
      }
      return supportsDecompressionStream ? browserInflate(buffer) : pako.inflate(buffer);
    }
    async function browserInflate(buffer) {
      const ds = new DecompressionStream("deflate");
      const d = new Blob([buffer]).stream().pipeThrough(ds);
      return new Uint8Array(await new Response(d).arrayBuffer());
    }
    function testDecompressionStream() {
      try {
        const ds = new DecompressionStream("deflate");
        if (ds) return true;
      } catch (_) {
      }
      return false;
    }
    function decodeVarInt(reader) {
      const bytes = [];
      let byte = 0;
      let multibyte = 0;
      do {
        byte = reader.readUInt8();
        const lastSeven = byte & 127;
        bytes.push(lastSeven);
        multibyte = byte & 128;
      } while (multibyte);
      return bytes.reduce((a, b) => a + 1 << 7 | b, -1);
    }
    function otherVarIntDecode(reader, startWith) {
      let result = startWith;
      let shift = 4;
      let byte = null;
      do {
        byte = reader.readUInt8();
        result |= (byte & 127) << shift;
        shift += 7;
      } while (byte & 128);
      return result;
    }
    var GitPackIndex = class _GitPackIndex {
      constructor(stuff) {
        Object.assign(this, stuff);
        this.offsetCache = {};
      }
      static async fromIdx({ idx, getExternalRefDelta }) {
        const reader = new BufferCursor(idx);
        const magic = reader.slice(4).toString("hex");
        if (magic !== "ff744f63") {
          return;
        }
        const version2 = reader.readUInt32BE();
        if (version2 !== 2) {
          throw new InternalError(
            `Unable to read version ${version2} packfile IDX. (Only version 2 supported)`
          );
        }
        if (idx.byteLength > 2048 * 1024 * 1024) {
          throw new InternalError(
            `To keep implementation simple, I haven't implemented the layer 5 feature needed to support packfiles > 2GB in size.`
          );
        }
        reader.seek(reader.tell() + 4 * 255);
        const size = reader.readUInt32BE();
        const hashes = [];
        for (let i = 0; i < size; i++) {
          const hash = reader.slice(20).toString("hex");
          hashes[i] = hash;
        }
        reader.seek(reader.tell() + 4 * size);
        const offsets = /* @__PURE__ */ new Map();
        for (let i = 0; i < size; i++) {
          offsets.set(hashes[i], reader.readUInt32BE());
        }
        const packfileSha = reader.slice(20).toString("hex");
        return new _GitPackIndex({
          hashes,
          crcs: {},
          offsets,
          packfileSha,
          getExternalRefDelta
        });
      }
      static async fromPack({ pack, getExternalRefDelta, onProgress }) {
        const listpackTypes = {
          1: "commit",
          2: "tree",
          3: "blob",
          4: "tag",
          6: "ofs-delta",
          7: "ref-delta"
        };
        const offsetToObject = {};
        const packfileSha = pack.slice(-20).toString("hex");
        const hashes = [];
        const crcs = {};
        const offsets = /* @__PURE__ */ new Map();
        let totalObjectCount = null;
        let lastPercent = null;
        await listpack([pack], async ({ data, type, reference, offset, num: num2 }) => {
          if (totalObjectCount === null) totalObjectCount = num2;
          const percent = Math.floor(
            (totalObjectCount - num2) * 100 / totalObjectCount
          );
          if (percent !== lastPercent) {
            if (onProgress) {
              await onProgress({
                phase: "Receiving objects",
                loaded: totalObjectCount - num2,
                total: totalObjectCount
              });
            }
          }
          lastPercent = percent;
          type = listpackTypes[type];
          if (["commit", "tree", "blob", "tag"].includes(type)) {
            offsetToObject[offset] = {
              type,
              offset
            };
          } else if (type === "ofs-delta") {
            offsetToObject[offset] = {
              type,
              offset
            };
          } else if (type === "ref-delta") {
            offsetToObject[offset] = {
              type,
              offset
            };
          }
        });
        const offsetArray = Object.keys(offsetToObject).map(Number);
        for (const [i, start] of offsetArray.entries()) {
          const end = i + 1 === offsetArray.length ? pack.byteLength - 20 : offsetArray[i + 1];
          const o = offsetToObject[start];
          const crc = crc32.buf(pack.slice(start, end)) >>> 0;
          o.end = end;
          o.crc = crc;
        }
        const p = new _GitPackIndex({
          pack: Promise.resolve(pack),
          packfileSha,
          crcs,
          hashes,
          offsets,
          getExternalRefDelta
        });
        lastPercent = null;
        let count = 0;
        const objectsByDepth = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        for (let offset in offsetToObject) {
          offset = Number(offset);
          const percent = Math.floor(count * 100 / totalObjectCount);
          if (percent !== lastPercent) {
            if (onProgress) {
              await onProgress({
                phase: "Resolving deltas",
                loaded: count,
                total: totalObjectCount
              });
            }
          }
          count++;
          lastPercent = percent;
          const o = offsetToObject[offset];
          if (o.oid) continue;
          try {
            p.readDepth = 0;
            p.externalReadDepth = 0;
            const { type, object } = await p.readSlice({ start: offset });
            objectsByDepth[p.readDepth] += 1;
            const oid = await shasum(GitObject.wrap({ type, object }));
            o.oid = oid;
            hashes.push(oid);
            offsets.set(oid, offset);
            crcs[oid] = o.crc;
          } catch (err) {
            continue;
          }
        }
        hashes.sort();
        return p;
      }
      async toBuffer() {
        const buffers = [];
        const write = (str, encoding) => {
          buffers.push(Buffer.from(str, encoding));
        };
        write("ff744f63", "hex");
        write("00000002", "hex");
        const fanoutBuffer = new BufferCursor(Buffer.alloc(256 * 4));
        for (let i = 0; i < 256; i++) {
          let count = 0;
          for (const hash of this.hashes) {
            if (parseInt(hash.slice(0, 2), 16) <= i) count++;
          }
          fanoutBuffer.writeUInt32BE(count);
        }
        buffers.push(fanoutBuffer.buffer);
        for (const hash of this.hashes) {
          write(hash, "hex");
        }
        const crcsBuffer = new BufferCursor(Buffer.alloc(this.hashes.length * 4));
        for (const hash of this.hashes) {
          crcsBuffer.writeUInt32BE(this.crcs[hash]);
        }
        buffers.push(crcsBuffer.buffer);
        const offsetsBuffer = new BufferCursor(Buffer.alloc(this.hashes.length * 4));
        for (const hash of this.hashes) {
          offsetsBuffer.writeUInt32BE(this.offsets.get(hash));
        }
        buffers.push(offsetsBuffer.buffer);
        write(this.packfileSha, "hex");
        const totalBuffer = Buffer.concat(buffers);
        const sha = await shasum(totalBuffer);
        const shaBuffer = Buffer.alloc(20);
        shaBuffer.write(sha, "hex");
        return Buffer.concat([totalBuffer, shaBuffer]);
      }
      async load({ pack }) {
        this.pack = pack;
      }
      async unload() {
        this.pack = null;
      }
      async read({ oid }) {
        if (!this.offsets.get(oid)) {
          if (this.getExternalRefDelta) {
            this.externalReadDepth++;
            return this.getExternalRefDelta(oid);
          } else {
            throw new InternalError(`Could not read object ${oid} from packfile`);
          }
        }
        const start = this.offsets.get(oid);
        return this.readSlice({ start });
      }
      async readSlice({ start }) {
        if (this.offsetCache[start]) {
          return Object.assign({}, this.offsetCache[start]);
        }
        this.readDepth++;
        const types2 = {
          16: "commit",
          32: "tree",
          48: "blob",
          64: "tag",
          96: "ofs_delta",
          112: "ref_delta"
        };
        const pack = await this.pack;
        if (!pack) {
          throw new InternalError(
            "Could not read packfile data. The packfile may be missing, corrupted, or too large to read into memory."
          );
        }
        const raw = pack.slice(start);
        const reader = new BufferCursor(raw);
        const byte = reader.readUInt8();
        const btype = byte & 112;
        let type = types2[btype];
        if (type === void 0) {
          throw new InternalError("Unrecognized type: 0b" + btype.toString(2));
        }
        const lastFour = byte & 15;
        let length = lastFour;
        const multibyte = byte & 128;
        if (multibyte) {
          length = otherVarIntDecode(reader, lastFour);
        }
        let base = null;
        let object = null;
        if (type === "ofs_delta") {
          const offset = decodeVarInt(reader);
          const baseOffset = start - offset;
          ({ object: base, type } = await this.readSlice({ start: baseOffset }));
        }
        if (type === "ref_delta") {
          const oid = reader.slice(20).toString("hex");
          ({ object: base, type } = await this.read({ oid }));
        }
        const buffer = raw.slice(reader.tell());
        object = Buffer.from(await inflate(buffer));
        if (object.byteLength !== length) {
          throw new InternalError(
            `Packfile told us object would have length ${length} but it had length ${object.byteLength}`
          );
        }
        if (base) {
          object = Buffer.from(applyDelta(object, base));
        }
        if (this.readDepth > 3) {
          this.offsetCache[start] = { type, object };
        }
        return { type, format: "content", object };
      }
    };
    var PackfileCache = Symbol("PackfileCache");
    async function loadPackIndex({
      fs: fs3,
      filename,
      getExternalRefDelta,
      emitter,
      emitterPrefix
    }) {
      const idx = await fs3.read(filename);
      return GitPackIndex.fromIdx({ idx, getExternalRefDelta });
    }
    function readPackIndex({
      fs: fs3,
      cache,
      filename,
      getExternalRefDelta,
      emitter,
      emitterPrefix
    }) {
      if (!cache[PackfileCache]) cache[PackfileCache] = /* @__PURE__ */ new Map();
      let p = cache[PackfileCache].get(filename);
      if (!p) {
        p = loadPackIndex({
          fs: fs3,
          filename,
          getExternalRefDelta,
          emitter,
          emitterPrefix
        });
        cache[PackfileCache].set(filename, p);
      }
      return p;
    }
    var SHA1_CHUNK_SIZE = 8 * 1024 * 1024;
    async function shasumRange(buffer, { start = 0, end = buffer.length } = {}) {
      const hash = crypto$1.createHash("sha1");
      for (let i = start; i < end; i += SHA1_CHUNK_SIZE) {
        hash.update(buffer.subarray(i, Math.min(i + SHA1_CHUNK_SIZE, end)));
      }
      return hash.digest("hex");
    }
    async function readObjectPacked({
      fs: fs3,
      cache,
      gitdir,
      oid,
      format = "content",
      getExternalRefDelta
    }) {
      let list = await fs3.readdir(join2(gitdir, "objects/pack"));
      list = list.filter((x) => x.endsWith(".idx"));
      for (const filename of list) {
        const indexFile = `${gitdir}/objects/pack/${filename}`;
        const p = await readPackIndex({
          fs: fs3,
          cache,
          filename: indexFile,
          getExternalRefDelta
        });
        if (p.error) throw new InternalError(p.error);
        if (p.offsets.has(oid)) {
          const packFile = indexFile.replace(/idx$/, "pack");
          if (!p.pack) {
            p.pack = fs3.read(packFile);
          }
          const pack = await p.pack;
          if (!pack) {
            p.pack = null;
            throw new InternalError(
              `Could not read packfile at ${packFile}. The file may be missing, corrupted, or too large to read into memory.`
            );
          }
          if (!p._checksumVerified) {
            const expectedShaFromIndex = p.packfileSha;
            const packTrailer = pack.subarray(-20);
            const packTrailerSha = Array.from(packTrailer).map((b) => b.toString(16).padStart(2, "0")).join("");
            if (packTrailerSha !== expectedShaFromIndex) {
              throw new InternalError(
                `Packfile trailer mismatch: expected ${expectedShaFromIndex}, got ${packTrailerSha}. The packfile may be corrupted.`
              );
            }
            const actualPayloadSha = await shasumRange(pack, {
              start: 0,
              end: pack.length - 20
            });
            if (actualPayloadSha !== expectedShaFromIndex) {
              throw new InternalError(
                `Packfile payload corrupted: calculated ${actualPayloadSha} but expected ${expectedShaFromIndex}. The packfile may have been tampered with.`
              );
            }
            p._checksumVerified = true;
          }
          const result = await p.read({ oid, getExternalRefDelta });
          result.format = "content";
          result.source = `objects/pack/${filename.replace(/idx$/, "pack")}`;
          return result;
        }
      }
      return null;
    }
    async function _readObject({
      fs: fs3,
      cache,
      gitdir,
      oid,
      format = "content"
    }) {
      const getExternalRefDelta = (oid2) => _readObject({ fs: fs3, cache, gitdir, oid: oid2 });
      let result;
      if (oid === "4b825dc642cb6eb9a060e54bf8d69288fbee4904") {
        result = { format: "wrapped", object: Buffer.from(`tree 0\0`) };
      }
      if (!result) {
        result = await readObjectLoose({ fs: fs3, gitdir, oid });
      }
      if (!result) {
        result = await readObjectPacked({
          fs: fs3,
          cache,
          gitdir,
          oid,
          getExternalRefDelta
        });
        if (!result) {
          throw new NotFoundError(oid);
        }
        return result;
      }
      if (format === "deflated") {
        return result;
      }
      if (result.format === "deflated") {
        result.object = Buffer.from(await inflate(result.object));
        result.format = "wrapped";
      }
      if (format === "wrapped") {
        return result;
      }
      const sha = await shasum(result.object);
      if (sha !== oid) {
        throw new InternalError(
          `SHA check failed! Expected ${oid}, computed ${sha}`
        );
      }
      const { object, type } = GitObject.unwrap(result.object);
      result.type = type;
      result.object = object;
      result.format = "content";
      if (format === "content") {
        return result;
      }
      throw new InternalError(`invalid requested format "${format}"`);
    }
    var AlreadyExistsError = class _AlreadyExistsError extends BaseError {
      /**
       * @param {'note'|'remote'|'tag'|'branch'} noun
       * @param {string} where
       * @param {boolean} canForce
       */
      constructor(noun, where, canForce = true) {
        super(
          `Failed to create ${noun} at ${where} because it already exists.${canForce ? ` (Hint: use 'force: true' parameter to overwrite existing ${noun}.)` : ""}`
        );
        this.code = this.name = _AlreadyExistsError.code;
        this.data = { noun, where, canForce };
      }
    };
    AlreadyExistsError.code = "AlreadyExistsError";
    var AmbiguousError = class _AmbiguousError extends BaseError {
      /**
       * @param {'oids'|'refs'} nouns
       * @param {string} short
       * @param {string[]} matches
       */
      constructor(nouns, short, matches) {
        super(
          `Found multiple ${nouns} matching "${short}" (${matches.join(
            ", "
          )}). Use a longer abbreviation length to disambiguate them.`
        );
        this.code = this.name = _AmbiguousError.code;
        this.data = { nouns, short, matches };
      }
    };
    AmbiguousError.code = "AmbiguousError";
    var CheckoutConflictError = class _CheckoutConflictError extends BaseError {
      /**
       * @param {string[]} filepaths
       */
      constructor(filepaths) {
        super(
          `Your local changes to the following files would be overwritten by checkout: ${filepaths.join(
            ", "
          )}`
        );
        this.code = this.name = _CheckoutConflictError.code;
        this.data = { filepaths };
      }
    };
    CheckoutConflictError.code = "CheckoutConflictError";
    var CherryPickMergeCommitError = class _CherryPickMergeCommitError extends BaseError {
      /**
       * @param {string} oid
       * @param {number} parentCount
       */
      constructor(oid, parentCount) {
        super(
          `Cannot cherry-pick merge commit ${oid}. Merge commits have ${parentCount} parents and require specifying which parent to use as the base.`
        );
        this.code = this.name = _CherryPickMergeCommitError.code;
        this.data = { oid, parentCount };
      }
    };
    CherryPickMergeCommitError.code = "CherryPickMergeCommitError";
    var CherryPickRootCommitError = class _CherryPickRootCommitError extends BaseError {
      /**
       * @param {string} oid
       */
      constructor(oid) {
        super(
          `Cannot cherry-pick root commit ${oid}. Root commits have no parents.`
        );
        this.code = this.name = _CherryPickRootCommitError.code;
        this.data = { oid };
      }
    };
    CherryPickRootCommitError.code = "CherryPickRootCommitError";
    var CommitNotFetchedError = class _CommitNotFetchedError extends BaseError {
      /**
       * @param {string} ref
       * @param {string} oid
       */
      constructor(ref, oid) {
        super(
          `Failed to checkout "${ref}" because commit ${oid} is not available locally. Do a git fetch to make the branch available locally.`
        );
        this.code = this.name = _CommitNotFetchedError.code;
        this.data = { ref, oid };
      }
    };
    CommitNotFetchedError.code = "CommitNotFetchedError";
    var EmptyServerResponseError = class _EmptyServerResponseError extends BaseError {
      constructor() {
        super(`Empty response from git server.`);
        this.code = this.name = _EmptyServerResponseError.code;
        this.data = {};
      }
    };
    EmptyServerResponseError.code = "EmptyServerResponseError";
    var FastForwardError = class _FastForwardError extends BaseError {
      constructor() {
        super(`A simple fast-forward merge was not possible.`);
        this.code = this.name = _FastForwardError.code;
        this.data = {};
      }
    };
    FastForwardError.code = "FastForwardError";
    var GitPushError = class _GitPushError extends BaseError {
      /**
       * @param {string} prettyDetails
       * @param {PushResult} result
       */
      constructor(prettyDetails, result) {
        super(`One or more branches were not updated: ${prettyDetails}`);
        this.code = this.name = _GitPushError.code;
        this.data = { prettyDetails, result };
      }
    };
    GitPushError.code = "GitPushError";
    var HttpError = class _HttpError extends BaseError {
      /**
       * @param {number} statusCode
       * @param {string} statusMessage
       * @param {string} response
       */
      constructor(statusCode, statusMessage, response) {
        super(`HTTP Error: ${statusCode} ${statusMessage}`);
        this.code = this.name = _HttpError.code;
        this.data = { statusCode, statusMessage, response };
      }
    };
    HttpError.code = "HttpError";
    var InvalidFilepathError = class _InvalidFilepathError extends BaseError {
      /**
       * @param {'leading-slash'|'trailing-slash'|'directory'} [reason]
       */
      constructor(reason) {
        let message = "invalid filepath";
        if (reason === "leading-slash" || reason === "trailing-slash") {
          message = `"filepath" parameter should not include leading or trailing directory separators because these can cause problems on some platforms.`;
        } else if (reason === "directory") {
          message = `"filepath" should not be a directory.`;
        }
        super(message);
        this.code = this.name = _InvalidFilepathError.code;
        this.data = { reason };
      }
    };
    InvalidFilepathError.code = "InvalidFilepathError";
    var InvalidRefNameError = class _InvalidRefNameError extends BaseError {
      /**
       * @param {string} ref
       * @param {string} suggestion
       * @param {boolean} canForce
       */
      constructor(ref, suggestion) {
        super(
          `"${ref}" would be an invalid git reference. (Hint: a valid alternative would be "${suggestion}".)`
        );
        this.code = this.name = _InvalidRefNameError.code;
        this.data = { ref, suggestion };
      }
    };
    InvalidRefNameError.code = "InvalidRefNameError";
    var MaxDepthError = class _MaxDepthError extends BaseError {
      /**
       * @param {number} depth
       */
      constructor(depth) {
        super(`Maximum search depth of ${depth} exceeded.`);
        this.code = this.name = _MaxDepthError.code;
        this.data = { depth };
      }
    };
    MaxDepthError.code = "MaxDepthError";
    var MergeNotSupportedError = class _MergeNotSupportedError extends BaseError {
      constructor() {
        super(`Merges with conflicts are not supported yet.`);
        this.code = this.name = _MergeNotSupportedError.code;
        this.data = {};
      }
    };
    MergeNotSupportedError.code = "MergeNotSupportedError";
    var MergeConflictError = class _MergeConflictError extends BaseError {
      /**
       * @param {Array<string>} filepaths
       * @param {Array<string>} bothModified
       * @param {Array<string>} deleteByUs
       * @param {Array<string>} deleteByTheirs
       */
      constructor(filepaths, bothModified, deleteByUs, deleteByTheirs) {
        super(
          `Automatic merge failed with one or more merge conflicts in the following files: ${filepaths.toString()}. Fix conflicts then commit the result.`
        );
        this.code = this.name = _MergeConflictError.code;
        this.data = { filepaths, bothModified, deleteByUs, deleteByTheirs };
      }
    };
    MergeConflictError.code = "MergeConflictError";
    var MissingNameError = class _MissingNameError extends BaseError {
      /**
       * @param {'author'|'committer'|'tagger'} role
       */
      constructor(role) {
        super(
          `No name was provided for ${role} in the argument or in the .git/config file.`
        );
        this.code = this.name = _MissingNameError.code;
        this.data = { role };
      }
    };
    MissingNameError.code = "MissingNameError";
    var MissingParameterError = class _MissingParameterError extends BaseError {
      /**
       * @param {string} parameter
       */
      constructor(parameter) {
        super(
          `The function requires a "${parameter}" parameter but none was provided.`
        );
        this.code = this.name = _MissingParameterError.code;
        this.data = { parameter };
      }
    };
    MissingParameterError.code = "MissingParameterError";
    var MultipleGitError = class _MultipleGitError extends BaseError {
      /**
       * @param {Error[]} errors
       * @param {string} message
       */
      constructor(errors) {
        super(
          `There are multiple errors that were thrown by the method. Please refer to the "errors" property to see more`
        );
        this.code = this.name = _MultipleGitError.code;
        this.data = { errors };
        this.errors = errors;
      }
    };
    MultipleGitError.code = "MultipleGitError";
    var ParseError = class _ParseError extends BaseError {
      /**
       * @param {string} expected
       * @param {string} actual
       */
      constructor(expected, actual) {
        super(`Expected "${expected}" but received "${actual}".`);
        this.code = this.name = _ParseError.code;
        this.data = { expected, actual };
      }
    };
    ParseError.code = "ParseError";
    var PushRejectedError = class _PushRejectedError extends BaseError {
      /**
       * @param {'not-fast-forward'|'tag-exists'} reason
       */
      constructor(reason) {
        let message = "";
        if (reason === "not-fast-forward") {
          message = " because it was not a simple fast-forward";
        } else if (reason === "tag-exists") {
          message = " because tag already exists";
        }
        super(`Push rejected${message}. Use "force: true" to override.`);
        this.code = this.name = _PushRejectedError.code;
        this.data = { reason };
      }
    };
    PushRejectedError.code = "PushRejectedError";
    var RemoteCapabilityError = class _RemoteCapabilityError extends BaseError {
      /**
       * @param {'shallow'|'deepen-since'|'deepen-not'|'deepen-relative'} capability
       * @param {'depth'|'since'|'exclude'|'relative'} parameter
       */
      constructor(capability, parameter) {
        super(
          `Remote does not support the "${capability}" so the "${parameter}" parameter cannot be used.`
        );
        this.code = this.name = _RemoteCapabilityError.code;
        this.data = { capability, parameter };
      }
    };
    RemoteCapabilityError.code = "RemoteCapabilityError";
    var SmartHttpError = class _SmartHttpError extends BaseError {
      /**
       * @param {string} preview
       * @param {string} response
       */
      constructor(preview, response) {
        super(
          `Remote did not reply using the "smart" HTTP protocol. Expected "001e# service=git-upload-pack" but received: ${preview}`
        );
        this.code = this.name = _SmartHttpError.code;
        this.data = { preview, response };
      }
    };
    SmartHttpError.code = "SmartHttpError";
    var UnknownTransportError = class _UnknownTransportError extends BaseError {
      /**
       * @param {string} url
       * @param {string} transport
       * @param {string} [suggestion]
       */
      constructor(url, transport, suggestion) {
        super(
          `Git remote "${url}" uses an unrecognized transport protocol: "${transport}"`
        );
        this.code = this.name = _UnknownTransportError.code;
        this.data = { url, transport, suggestion };
      }
    };
    UnknownTransportError.code = "UnknownTransportError";
    var UrlParseError = class _UrlParseError extends BaseError {
      /**
       * @param {string} url
       */
      constructor(url) {
        super(`Cannot parse remote URL: "${url}"`);
        this.code = this.name = _UrlParseError.code;
        this.data = { url };
      }
    };
    UrlParseError.code = "UrlParseError";
    var UserCanceledError = class _UserCanceledError extends BaseError {
      constructor() {
        super(`The operation was canceled.`);
        this.code = this.name = _UserCanceledError.code;
        this.data = {};
      }
    };
    UserCanceledError.code = "UserCanceledError";
    var IndexResetError = class _IndexResetError extends BaseError {
      /**
       * @param {Array<string>} filepaths
       */
      constructor(filepath) {
        super(
          `Could not merge index: Entry for '${filepath}' is not up to date. Either reset the index entry to HEAD, or stage your unstaged changes.`
        );
        this.code = this.name = _IndexResetError.code;
        this.data = { filepath };
      }
    };
    IndexResetError.code = "IndexResetError";
    var NoCommitError = class _NoCommitError extends BaseError {
      /**
       * @param {string} ref
       */
      constructor(ref) {
        super(
          `"${ref}" does not point to any commit. You're maybe working on a repository with no commits yet. `
        );
        this.code = this.name = _NoCommitError.code;
        this.data = { ref };
      }
    };
    NoCommitError.code = "NoCommitError";
    var Errors = /* @__PURE__ */ Object.freeze({
      __proto__: null,
      AlreadyExistsError,
      AmbiguousError,
      CheckoutConflictError,
      CherryPickMergeCommitError,
      CherryPickRootCommitError,
      CommitNotFetchedError,
      EmptyServerResponseError,
      FastForwardError,
      GitPushError,
      HttpError,
      InternalError,
      InvalidFilepathError,
      InvalidOidError,
      InvalidRefNameError,
      MaxDepthError,
      MergeNotSupportedError,
      MergeConflictError,
      MissingNameError,
      MissingParameterError,
      MultipleGitError,
      NoRefspecError,
      NotFoundError,
      ObjectTypeError,
      ParseError,
      PushRejectedError,
      RemoteCapabilityError,
      SmartHttpError,
      UnknownTransportError,
      UnsafeFilepathError,
      UrlParseError,
      UserCanceledError,
      UnmergedPathsError,
      IndexResetError,
      NoCommitError
    });
    function formatAuthor({ name, email, timestamp, timezoneOffset }) {
      timezoneOffset = formatTimezoneOffset(timezoneOffset);
      return `${name} <${email}> ${timestamp} ${timezoneOffset}`;
    }
    function formatTimezoneOffset(minutes) {
      const sign = simpleSign(negateExceptForZero(minutes));
      minutes = Math.abs(minutes);
      const hours = Math.floor(minutes / 60);
      minutes -= hours * 60;
      let strHours = String(hours);
      let strMinutes = String(minutes);
      if (strHours.length < 2) strHours = "0" + strHours;
      if (strMinutes.length < 2) strMinutes = "0" + strMinutes;
      return (sign === -1 ? "-" : "+") + strHours + strMinutes;
    }
    function simpleSign(n) {
      return Math.sign(n) || (Object.is(n, -0) ? -1 : 1);
    }
    function negateExceptForZero(n) {
      return n === 0 ? n : -n;
    }
    function normalizeNewlines(str) {
      str = str.replace(/\r/g, "");
      str = str.replace(/^\n+/, "");
      str = str.replace(/\n+$/, "") + "\n";
      return str;
    }
    function parseAuthor(author) {
      const [, name, email, timestamp, offset] = author.match(
        /^(.*) <(.*)> (.*) (.*)$/
      );
      return {
        name,
        email,
        timestamp: Number(timestamp),
        timezoneOffset: parseTimezoneOffset(offset)
      };
    }
    function parseTimezoneOffset(offset) {
      let [, sign, hours, minutes] = offset.match(/(\+|-)(\d\d)(\d\d)/);
      minutes = (sign === "+" ? 1 : -1) * (Number(hours) * 60 + Number(minutes));
      return negateExceptForZero$1(minutes);
    }
    function negateExceptForZero$1(n) {
      return n === 0 ? n : -n;
    }
    var GitAnnotatedTag = class _GitAnnotatedTag {
      constructor(tag2) {
        if (typeof tag2 === "string") {
          this._tag = tag2;
        } else if (Buffer.isBuffer(tag2)) {
          this._tag = tag2.toString("utf8");
        } else if (typeof tag2 === "object") {
          this._tag = _GitAnnotatedTag.render(tag2);
        } else {
          throw new InternalError(
            "invalid type passed to GitAnnotatedTag constructor"
          );
        }
      }
      static from(tag2) {
        return new _GitAnnotatedTag(tag2);
      }
      static render(obj) {
        return `object ${obj.object}
type ${obj.type}
tag ${obj.tag}
tagger ${formatAuthor(obj.tagger)}

${obj.message}
${obj.gpgsig ? obj.gpgsig : ""}`;
      }
      justHeaders() {
        return this._tag.slice(0, this._tag.indexOf("\n\n"));
      }
      message() {
        const tag2 = this.withoutSignature();
        return tag2.slice(tag2.indexOf("\n\n") + 2);
      }
      parse() {
        return Object.assign(this.headers(), {
          message: this.message(),
          gpgsig: this.gpgsig()
        });
      }
      render() {
        return this._tag;
      }
      headers() {
        const headers = this.justHeaders().split("\n");
        const hs = [];
        for (const h of headers) {
          if (h[0] === " ") {
            hs[hs.length - 1] += "\n" + h.slice(1);
          } else {
            hs.push(h);
          }
        }
        const obj = {};
        for (const h of hs) {
          const key = h.slice(0, h.indexOf(" "));
          const value = h.slice(h.indexOf(" ") + 1);
          if (Array.isArray(obj[key])) {
            obj[key].push(value);
          } else {
            obj[key] = value;
          }
        }
        if (obj.tagger) {
          obj.tagger = parseAuthor(obj.tagger);
        }
        if (obj.committer) {
          obj.committer = parseAuthor(obj.committer);
        }
        return obj;
      }
      withoutSignature() {
        const tag2 = normalizeNewlines(this._tag);
        if (tag2.indexOf("\n-----BEGIN PGP SIGNATURE-----") === -1) return tag2;
        return tag2.slice(0, tag2.lastIndexOf("\n-----BEGIN PGP SIGNATURE-----"));
      }
      gpgsig() {
        if (this._tag.indexOf("\n-----BEGIN PGP SIGNATURE-----") === -1) return;
        const signature = this._tag.slice(
          this._tag.indexOf("-----BEGIN PGP SIGNATURE-----"),
          this._tag.indexOf("-----END PGP SIGNATURE-----") + "-----END PGP SIGNATURE-----".length
        );
        return normalizeNewlines(signature);
      }
      payload() {
        return this.withoutSignature() + "\n";
      }
      toObject() {
        return Buffer.from(this._tag, "utf8");
      }
      static async sign(tag2, sign, secretKey) {
        const payload = tag2.payload();
        let { signature } = await sign({ payload, secretKey });
        signature = normalizeNewlines(signature);
        const signedTag = payload + signature;
        return _GitAnnotatedTag.from(signedTag);
      }
    };
    function indent(str) {
      return str.trim().split("\n").map((x) => " " + x).join("\n") + "\n";
    }
    function outdent(str) {
      return str.split("\n").map((x) => x.replace(/^ /, "")).join("\n");
    }
    var GitCommit = class _GitCommit {
      constructor(commit2) {
        if (typeof commit2 === "string") {
          this._commit = commit2;
        } else if (Buffer.isBuffer(commit2)) {
          this._commit = commit2.toString("utf8");
        } else if (typeof commit2 === "object") {
          this._commit = _GitCommit.render(commit2);
        } else {
          throw new InternalError("invalid type passed to GitCommit constructor");
        }
      }
      static fromPayloadSignature({ payload, signature }) {
        const headers = _GitCommit.justHeaders(payload);
        const message = _GitCommit.justMessage(payload);
        const commit2 = normalizeNewlines(
          headers + "\ngpgsig" + indent(signature) + "\n" + message
        );
        return new _GitCommit(commit2);
      }
      static from(commit2) {
        return new _GitCommit(commit2);
      }
      toObject() {
        return Buffer.from(this._commit, "utf8");
      }
      // Todo: allow setting the headers and message
      headers() {
        return this.parseHeaders();
      }
      // Todo: allow setting the headers and message
      message() {
        return _GitCommit.justMessage(this._commit);
      }
      parse() {
        return Object.assign({ message: this.message() }, this.headers());
      }
      static justMessage(commit2) {
        return normalizeNewlines(commit2.slice(commit2.indexOf("\n\n") + 2));
      }
      static justHeaders(commit2) {
        return commit2.slice(0, commit2.indexOf("\n\n"));
      }
      parseHeaders() {
        const headers = _GitCommit.justHeaders(this._commit).split("\n");
        const hs = [];
        for (const h of headers) {
          if (h[0] === " ") {
            hs[hs.length - 1] += "\n" + h.slice(1);
          } else {
            hs.push(h);
          }
        }
        const obj = {
          parent: []
        };
        for (const h of hs) {
          const key = h.slice(0, h.indexOf(" "));
          const value = h.slice(h.indexOf(" ") + 1);
          if (Array.isArray(obj[key])) {
            obj[key].push(value);
          } else {
            obj[key] = value;
          }
        }
        if (obj.author) {
          obj.author = parseAuthor(obj.author);
        }
        if (obj.committer) {
          obj.committer = parseAuthor(obj.committer);
        }
        return obj;
      }
      static renderHeaders(obj) {
        let headers = "";
        if (obj.tree) {
          headers += `tree ${obj.tree}
`;
        } else {
          headers += `tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904
`;
        }
        if (obj.parent) {
          if (obj.parent.length === void 0) {
            throw new InternalError(`commit 'parent' property should be an array`);
          }
          for (const p of obj.parent) {
            headers += `parent ${p}
`;
          }
        }
        const author = obj.author;
        headers += `author ${formatAuthor(author)}
`;
        const committer = obj.committer || obj.author;
        headers += `committer ${formatAuthor(committer)}
`;
        if (obj.gpgsig) {
          headers += "gpgsig" + indent(obj.gpgsig);
        }
        return headers;
      }
      static render(obj) {
        return _GitCommit.renderHeaders(obj) + "\n" + normalizeNewlines(obj.message);
      }
      render() {
        return this._commit;
      }
      withoutSignature() {
        const commit2 = normalizeNewlines(this._commit);
        if (commit2.indexOf("\ngpgsig") === -1) return commit2;
        const headers = commit2.slice(0, commit2.indexOf("\ngpgsig"));
        const message = commit2.slice(
          commit2.indexOf("-----END PGP SIGNATURE-----\n") + "-----END PGP SIGNATURE-----\n".length
        );
        return normalizeNewlines(headers + "\n" + message);
      }
      isolateSignature() {
        const signature = this._commit.slice(
          this._commit.indexOf("-----BEGIN PGP SIGNATURE-----"),
          this._commit.indexOf("-----END PGP SIGNATURE-----") + "-----END PGP SIGNATURE-----".length
        );
        return outdent(signature);
      }
      static async sign(commit2, sign, secretKey) {
        const payload = commit2.withoutSignature();
        const message = _GitCommit.justMessage(commit2._commit);
        let { signature } = await sign({ payload, secretKey });
        signature = normalizeNewlines(signature);
        const headers = _GitCommit.justHeaders(commit2._commit);
        const signedCommit = headers + "\ngpgsig" + indent(signature) + "\n" + message;
        return _GitCommit.from(signedCommit);
      }
    };
    async function resolveTree({ fs: fs3, cache, gitdir, oid }) {
      if (oid === "4b825dc642cb6eb9a060e54bf8d69288fbee4904") {
        return { tree: GitTree.from([]), oid };
      }
      const { type, object } = await _readObject({ fs: fs3, cache, gitdir, oid });
      if (type === "tag") {
        oid = GitAnnotatedTag.from(object).parse().object;
        return resolveTree({ fs: fs3, cache, gitdir, oid });
      }
      if (type === "commit") {
        oid = GitCommit.from(object).parse().tree;
        return resolveTree({ fs: fs3, cache, gitdir, oid });
      }
      if (type !== "tree") {
        throw new ObjectTypeError(oid, type, "tree");
      }
      return { tree: GitTree.from(object), oid };
    }
    var GitWalkerRepo = class {
      constructor({ fs: fs3, gitdir, ref, cache }) {
        this.fs = fs3;
        this.cache = cache;
        this.gitdir = gitdir;
        this.mapPromise = (async () => {
          const map = /* @__PURE__ */ new Map();
          let oid;
          try {
            oid = await GitRefManager.resolve({ fs: fs3, gitdir, ref });
          } catch (e) {
            if (e instanceof NotFoundError) {
              oid = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
            }
          }
          const tree = await resolveTree({ fs: fs3, cache: this.cache, gitdir, oid });
          tree.type = "tree";
          tree.mode = "40000";
          map.set(".", tree);
          return map;
        })();
        const walker = this;
        this.ConstructEntry = class TreeEntry {
          constructor(fullpath) {
            this._fullpath = fullpath;
            this._type = false;
            this._mode = false;
            this._stat = false;
            this._content = false;
            this._oid = false;
          }
          async type() {
            return walker.type(this);
          }
          async mode() {
            return walker.mode(this);
          }
          async stat() {
            return walker.stat(this);
          }
          async content() {
            return walker.content(this);
          }
          async oid() {
            return walker.oid(this);
          }
        };
      }
      async readdir(entry) {
        const filepath = entry._fullpath;
        const { fs: fs3, cache, gitdir } = this;
        const map = await this.mapPromise;
        const obj = map.get(filepath);
        if (!obj) throw new Error(`No obj for ${filepath}`);
        const oid = obj.oid;
        if (!oid) throw new Error(`No oid for obj ${JSON.stringify(obj)}`);
        if (obj.type !== "tree") {
          return null;
        }
        const { type, object } = await _readObject({ fs: fs3, cache, gitdir, oid });
        if (type !== obj.type) {
          throw new ObjectTypeError(oid, type, obj.type);
        }
        const tree = GitTree.from(object);
        for (const entry2 of tree) {
          map.set(join2(filepath, entry2.path), entry2);
        }
        return tree.entries().map((entry2) => join2(filepath, entry2.path));
      }
      async type(entry) {
        if (entry._type === false) {
          const map = await this.mapPromise;
          const { type } = map.get(entry._fullpath);
          entry._type = type;
        }
        return entry._type;
      }
      async mode(entry) {
        if (entry._mode === false) {
          const map = await this.mapPromise;
          const { mode } = map.get(entry._fullpath);
          entry._mode = normalizeMode(parseInt(mode, 8));
        }
        return entry._mode;
      }
      async stat(_entry) {
      }
      async content(entry) {
        if (entry._content === false) {
          const map = await this.mapPromise;
          const { fs: fs3, cache, gitdir } = this;
          const obj = map.get(entry._fullpath);
          const oid = obj.oid;
          const { type, object } = await _readObject({ fs: fs3, cache, gitdir, oid });
          if (type !== "blob") {
            entry._content = void 0;
          } else {
            entry._content = new Uint8Array(object);
          }
        }
        return entry._content;
      }
      async oid(entry) {
        if (entry._oid === false) {
          const map = await this.mapPromise;
          const obj = map.get(entry._fullpath);
          entry._oid = obj.oid;
        }
        return entry._oid;
      }
    };
    function TREE({ ref = "HEAD" } = {}) {
      const o = /* @__PURE__ */ Object.create(null);
      Object.defineProperty(o, GitWalkSymbol, {
        value: function({ fs: fs3, gitdir, cache }) {
          return new GitWalkerRepo({ fs: fs3, gitdir, ref, cache });
        }
      });
      Object.freeze(o);
      return o;
    }
    var GitWalkerFs = class {
      constructor({ fs: fs3, dir, gitdir, cache }) {
        this.fs = fs3;
        this.cache = cache;
        this.dir = dir;
        this.gitdir = gitdir;
        this.config = null;
        const walker = this;
        this.ConstructEntry = class WorkdirEntry {
          constructor(fullpath) {
            this._fullpath = fullpath;
            this._type = false;
            this._mode = false;
            this._stat = false;
            this._content = false;
            this._oid = false;
          }
          async type() {
            return walker.type(this);
          }
          async mode() {
            return walker.mode(this);
          }
          async stat() {
            return walker.stat(this);
          }
          async content() {
            return walker.content(this);
          }
          async oid() {
            return walker.oid(this);
          }
        };
      }
      async readdir(entry) {
        const filepath = entry._fullpath;
        const { fs: fs3, dir } = this;
        const names = await fs3.readdir(join2(dir, filepath));
        if (names === null) return null;
        return names.map((name) => join2(filepath, name));
      }
      async type(entry) {
        if (entry._type === false) {
          await entry.stat();
        }
        return entry._type;
      }
      async mode(entry) {
        if (entry._mode === false) {
          await entry.stat();
        }
        return entry._mode;
      }
      async stat(entry) {
        if (entry._stat === false) {
          const { fs: fs3, dir } = this;
          let stat = await fs3.lstat(`${dir}/${entry._fullpath}`);
          if (!stat) {
            throw new Error(
              `ENOENT: no such file or directory, lstat '${entry._fullpath}'`
            );
          }
          let type = stat.isDirectory() ? "tree" : "blob";
          if (type === "blob" && !stat.isFile() && !stat.isSymbolicLink()) {
            type = "special";
          }
          entry._type = type;
          stat = normalizeStats(stat);
          entry._mode = stat.mode;
          if (stat.size === -1 && entry._actualSize) {
            stat.size = entry._actualSize;
          }
          entry._stat = stat;
        }
        return entry._stat;
      }
      async content(entry) {
        if (entry._content === false) {
          const { fs: fs3, dir, gitdir } = this;
          if (await entry.type() === "tree") {
            entry._content = void 0;
          } else {
            let content;
            if (await entry.mode() >> 12 === 10) {
              content = await fs3.readlink(`${dir}/${entry._fullpath}`);
            } else {
              const config = await this._getGitConfig(fs3, gitdir);
              const autocrlf = await config.get("core.autocrlf");
              content = await fs3.read(`${dir}/${entry._fullpath}`, { autocrlf });
            }
            entry._actualSize = content.length;
            if (entry._stat && entry._stat.size === -1) {
              entry._stat.size = entry._actualSize;
            }
            entry._content = new Uint8Array(content);
          }
        }
        return entry._content;
      }
      async oid(entry) {
        if (entry._oid === false) {
          const self = this;
          const { fs: fs3, gitdir, cache } = this;
          let oid;
          await GitIndexManager.acquire(
            { fs: fs3, gitdir, cache },
            async function(index2) {
              const stage = index2.entriesMap.get(entry._fullpath);
              const stats = await entry.stat();
              const config = await self._getGitConfig(fs3, gitdir);
              const filemode = await config.get("core.filemode");
              const trustino = typeof process !== "undefined" ? !(process.platform === "win32") : true;
              if (!stage || compareStats(stats, stage, filemode, trustino)) {
                const content = await entry.content();
                if (content === void 0) {
                  oid = void 0;
                } else {
                  oid = await shasum(
                    GitObject.wrap({ type: "blob", object: content })
                  );
                  if (stage && oid === stage.oid && (!filemode || stats.mode === stage.mode) && compareStats(stats, stage, filemode, trustino)) {
                    index2.insert({
                      filepath: entry._fullpath,
                      stats,
                      oid
                    });
                  }
                }
              } else {
                oid = stage.oid;
              }
            }
          );
          entry._oid = oid;
        }
        return entry._oid;
      }
      async _getGitConfig(fs3, gitdir) {
        if (this.config) {
          return this.config;
        }
        this.config = await GitConfigManager.get({ fs: fs3, gitdir });
        return this.config;
      }
    };
    function WORKDIR() {
      const o = /* @__PURE__ */ Object.create(null);
      Object.defineProperty(o, GitWalkSymbol, {
        value: function({ fs: fs3, dir, gitdir, cache }) {
          return new GitWalkerFs({ fs: fs3, dir, gitdir, cache });
        }
      });
      Object.freeze(o);
      return o;
    }
    function arrayRange(start, end) {
      const length = end - start;
      return Array.from({ length }, (_, i) => start + i);
    }
    var flat = typeof Array.prototype.flat === "undefined" ? (entries) => entries.reduce((acc, x) => acc.concat(x), []) : (entries) => entries.flat();
    var RunningMinimum = class {
      constructor() {
        this.value = null;
      }
      consider(value) {
        if (value === null || value === void 0) return;
        if (this.value === null) {
          this.value = value;
        } else if (value < this.value) {
          this.value = value;
        }
      }
      reset() {
        this.value = null;
      }
    };
    function* unionOfIterators(sets) {
      const min = new RunningMinimum();
      let minimum;
      const heads = [];
      const numsets = sets.length;
      for (let i = 0; i < numsets; i++) {
        heads[i] = sets[i].next().value;
        if (heads[i] !== void 0) {
          min.consider(heads[i]);
        }
      }
      if (min.value === null) return;
      while (true) {
        const result = [];
        minimum = min.value;
        min.reset();
        for (let i = 0; i < numsets; i++) {
          if (heads[i] !== void 0 && heads[i] === minimum) {
            result[i] = heads[i];
            heads[i] = sets[i].next().value;
          } else {
            result[i] = null;
          }
          if (heads[i] !== void 0) {
            min.consider(heads[i]);
          }
        }
        yield result;
        if (min.value === null) return;
      }
    }
    async function _walk({
      fs: fs3,
      cache,
      dir,
      gitdir,
      trees,
      // @ts-ignore
      map = async (_, entry) => entry,
      // The default reducer is a flatmap that filters out undefineds.
      reduce = async (parent, children) => {
        const flatten = flat(children);
        if (parent !== void 0) flatten.unshift(parent);
        return flatten;
      },
      // The default iterate function walks all children concurrently
      iterate = (walk2, children) => Promise.all([...children].map(walk2))
    }) {
      const walkers = trees.map(
        (proxy) => proxy[GitWalkSymbol]({ fs: fs3, dir, gitdir, cache })
      );
      const root = new Array(walkers.length).fill(".");
      const range = arrayRange(0, walkers.length);
      const unionWalkerFromReaddir = async (entries) => {
        range.forEach((i) => {
          const entry = entries[i];
          entries[i] = entry && new walkers[i].ConstructEntry(entry);
        });
        const subdirs = await Promise.all(
          range.map((i) => {
            const entry = entries[i];
            return entry ? walkers[i].readdir(entry) : [];
          })
        );
        const iterators = subdirs.map((array) => {
          return (array === null ? [] : array)[Symbol.iterator]();
        });
        return {
          entries,
          children: unionOfIterators(iterators)
        };
      };
      const walk2 = async (root2) => {
        const { entries, children } = await unionWalkerFromReaddir(root2);
        const fullpath = entries.find((entry) => entry && entry._fullpath)._fullpath;
        const parent = await map(fullpath, entries);
        if (parent !== null) {
          let walkedChildren = await iterate(walk2, children);
          walkedChildren = walkedChildren.filter((x) => x !== void 0);
          return reduce(parent, walkedChildren);
        }
      };
      return walk2(root);
    }
    async function rmRecursive(fs3, filepath) {
      const entries = await fs3.readdir(filepath);
      if (entries == null) {
        await fs3.rm(filepath);
      } else if (entries.length) {
        await Promise.all(
          entries.map((entry) => {
            const subpath = join2(filepath, entry);
            return fs3.lstat(subpath).then((stat) => {
              if (!stat) return;
              return stat.isDirectory() ? rmRecursive(fs3, subpath) : fs3.rm(subpath);
            });
          })
        ).then(() => fs3.rmdir(filepath));
      } else {
        await fs3.rmdir(filepath);
      }
    }
    function isPromiseLike(obj) {
      return isObject(obj) && isFunction(obj.then) && isFunction(obj.catch);
    }
    function isObject(obj) {
      return obj && typeof obj === "object";
    }
    function isFunction(obj) {
      return typeof obj === "function";
    }
    function isPromiseFs(fs3) {
      const test = (targetFs) => {
        try {
          return targetFs.readFile().catch((e) => e);
        } catch (e) {
          return e;
        }
      };
      return isPromiseLike(test(fs3));
    }
    var commands = [
      "readFile",
      "writeFile",
      "mkdir",
      "rmdir",
      "unlink",
      "stat",
      "lstat",
      "readdir",
      "readlink",
      "symlink"
    ];
    function bindFs(target, fs3) {
      if (isPromiseFs(fs3)) {
        for (const command of commands) {
          target[`_${command}`] = fs3[command].bind(fs3);
        }
      } else {
        for (const command of commands) {
          target[`_${command}`] = pify(fs3[command].bind(fs3));
        }
      }
      if (isPromiseFs(fs3)) {
        if (fs3.cp) target._cp = fs3.cp.bind(fs3);
        if (fs3.rm) target._rm = fs3.rm.bind(fs3);
        else if (fs3.rmdir.length > 1) target._rm = fs3.rmdir.bind(fs3);
        else target._rm = rmRecursive.bind(null, target);
      } else {
        if (fs3.cp) target._cp = pify(fs3.cp.bind(fs3));
        if (fs3.rm) target._rm = pify(fs3.rm.bind(fs3));
        else if (fs3.rmdir.length > 2) target._rm = pify(fs3.rmdir.bind(fs3));
        else target._rm = rmRecursive.bind(null, target);
      }
    }
    var FileSystem = class {
      /**
       * Creates an instance of FileSystem.
       *
       * @param {Object} fs - A file system implementation to wrap.
       */
      constructor(fs3) {
        if (typeof fs3._original_unwrapped_fs !== "undefined") return fs3;
        const promises = Object.getOwnPropertyDescriptor(fs3, "promises");
        if (promises && promises.enumerable) {
          bindFs(this, fs3.promises);
        } else {
          bindFs(this, fs3);
        }
        this._original_unwrapped_fs = fs3;
      }
      /**
       * Return true if a file exists, false if it doesn't exist.
       * Rethrows errors that aren't related to file existence.
       *
       * @param {string} filepath - The path to the file.
       * @param {Object} [options] - Additional options.
       * @returns {Promise<boolean>} - `true` if the file exists, `false` otherwise.
       */
      async exists(filepath, options = {}) {
        try {
          await this._stat(filepath);
          return true;
        } catch (err) {
          if (err.code === "ENOENT" || err.code === "ENOTDIR" || (err.code || "").includes("ENS")) {
            return false;
          } else {
            console.log('Unhandled error in "FileSystem.exists()" function', err);
            throw err;
          }
        }
      }
      /**
       * Return the contents of a file if it exists, otherwise returns null.
       *
       * @param {string} filepath - The path to the file.
       * @param {Object} [options] - Options for reading the file.
       * @returns {Promise<Buffer|string|null>} - The file contents, or `null` if the file doesn't exist.
       */
      async read(filepath, options = {}) {
        try {
          let buffer = await this._readFile(filepath, options);
          if (options.autocrlf === "true") {
            try {
              buffer = new TextDecoder("utf8", { fatal: true }).decode(buffer);
              buffer = buffer.replace(/\r\n/g, "\n");
              buffer = new TextEncoder().encode(buffer);
            } catch (error) {
            }
          }
          if (typeof buffer !== "string") {
            buffer = Buffer.from(buffer);
          }
          return buffer;
        } catch (err) {
          return null;
        }
      }
      /**
       * Write a file (creating missing directories if need be) without throwing errors.
       *
       * @param {string} filepath - The path to the file.
       * @param {Buffer|Uint8Array|string} contents - The data to write.
       * @param {Object|string} [options] - Options for writing the file.
       * @returns {Promise<void>}
       */
      async write(filepath, contents, options = {}) {
        try {
          await this._writeFile(filepath, contents, options);
        } catch (err) {
          await this.mkdir(dirname(filepath));
          await this._writeFile(filepath, contents, options);
        }
      }
      /**
       * Make a directory (or series of nested directories) without throwing an error if it already exists.
       *
       * @param {string} filepath - The path to the directory.
       * @param {boolean} [_selfCall=false] - Internal flag to prevent infinite recursion.
       * @returns {Promise<void>}
       */
      async mkdir(filepath, _selfCall = false) {
        try {
          await this._mkdir(filepath);
        } catch (err) {
          if (err === null) return;
          if (err.code === "EEXIST") return;
          if (_selfCall) throw err;
          if (err.code === "ENOENT") {
            const parent = dirname(filepath);
            if (parent === "." || parent === "/" || parent === filepath) throw err;
            await this.mkdir(parent);
            await this.mkdir(filepath, true);
          }
        }
      }
      /**
       * Delete a file without throwing an error if it is already deleted.
       *
       * @param {string} filepath - The path to the file.
       * @returns {Promise<void>}
       */
      async rm(filepath) {
        try {
          await this._unlink(filepath);
        } catch (err) {
          if (err.code !== "ENOENT") throw err;
        }
      }
      /**
       * Delete a directory without throwing an error if it is already deleted.
       *
       * @param {string} filepath - The path to the directory.
       * @param {Object} [opts] - Options for deleting the directory.
       * @returns {Promise<void>}
       */
      async rmdir(filepath, opts) {
        try {
          if (opts && opts.recursive) {
            await this._rm(filepath, opts);
          } else {
            await this._rmdir(filepath);
          }
        } catch (err) {
          if (err.code !== "ENOENT") throw err;
        }
      }
      /**
       * Read a directory without throwing an error is the directory doesn't exist
       *
       * @param {string} filepath - The path to the directory.
       * @returns {Promise<string[]|null>} - An array of file names, or `null` if the path is not a directory.
       */
      async readdir(filepath) {
        try {
          const names = await this._readdir(filepath);
          names.sort(compareStrings);
          return names;
        } catch (err) {
          if (err.code === "ENOTDIR") return null;
          return [];
        }
      }
      /**
       * Return a flat list of all the files nested inside a directory
       *
       * Based on an elegant concurrent recursive solution from SO
       * https://stackoverflow.com/a/45130990/2168416
       *
       * @param {string} dir - The directory to read.
       * @returns {Promise<string[]>} - A flat list of all files in the directory.
       */
      async readdirDeep(dir) {
        const subdirs = await this._readdir(dir);
        const files = await Promise.all(
          subdirs.map(async (subdir) => {
            const res = dir + "/" + subdir;
            return (await this._stat(res)).isDirectory() ? this.readdirDeep(res) : res;
          })
        );
        return files.reduce((a, f) => a.concat(f), []);
      }
      /**
       * Return the Stats of a file/symlink if it exists, otherwise returns null.
       * Rethrows errors that aren't related to file existence.
       *
       * @param {string} filename - The path to the file or symlink.
       * @returns {Promise<Object|null>} - The stats object, or `null` if the file doesn't exist.
       */
      async lstat(filename) {
        try {
          const stats = await this._lstat(filename);
          return stats;
        } catch (err) {
          if (err.code === "ENOENT" || (err.code || "").includes("ENS")) {
            return null;
          }
          throw err;
        }
      }
      /**
       * Reads the contents of a symlink if it exists, otherwise returns null.
       * Rethrows errors that aren't related to file existence.
       *
       * @param {string} filename - The path to the symlink.
       * @param {Object} [opts={ encoding: 'buffer' }] - Options for reading the symlink.
       * @returns {Promise<Buffer|null>} - The symlink target, or `null` if it doesn't exist.
       */
      async readlink(filename, opts = { encoding: "buffer" }) {
        try {
          const link = await this._readlink(filename, opts);
          return Buffer.isBuffer(link) ? link : Buffer.from(link);
        } catch (err) {
          if (err.code === "ENOENT" || (err.code || "").includes("ENS")) {
            return null;
          }
          throw err;
        }
      }
      /**
       * Write the contents of buffer to a symlink.
       *
       * @param {string} filename - The path to the symlink.
       * @param {Buffer} buffer - The symlink target.
       * @returns {Promise<void>}
       */
      async writelink(filename, buffer) {
        return this._symlink(buffer.toString("utf8"), filename);
      }
    };
    function assertParameter(name, value) {
      if (value === void 0) {
        throw new MissingParameterError(name);
      }
    }
    function isAbsolute(filepath) {
      return filepath.startsWith("/") || /^[a-zA-Z]:[\\/]/.test(filepath);
    }
    async function discoverGitdir({ fsp, dotgit }) {
      assertParameter("fsp", fsp);
      assertParameter("dotgit", dotgit);
      const dotgitStat = await fsp._stat(dotgit).catch(() => ({ isFile: () => false, isDirectory: () => false }));
      if (dotgitStat.isDirectory()) {
        return dotgit;
      } else if (dotgitStat.isFile()) {
        return fsp._readFile(dotgit, "utf8").then((contents) => contents.trimRight().substr(8)).then((submoduleGitdir) => {
          if (isAbsolute(submoduleGitdir)) {
            return submoduleGitdir;
          }
          const gitdir = join2(dirname(dotgit), submoduleGitdir);
          return gitdir;
        });
      } else {
        return dotgit;
      }
    }
    async function modified(entry, base) {
      if (!entry && !base) return false;
      if (entry && !base) return true;
      if (!entry && base) return true;
      if (await entry.type() === "tree" && await base.type() === "tree") {
        return false;
      }
      if (await entry.type() === await base.type() && await entry.mode() === await base.mode() && await entry.oid() === await base.oid()) {
        return false;
      }
      return true;
    }
    async function abortMerge({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      commit: commit2 = "HEAD",
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("dir", dir);
        assertParameter("gitdir", gitdir);
        const fs3 = new FileSystem(_fs);
        const trees = [TREE({ ref: commit2 }), WORKDIR(), STAGE()];
        let unmergedPaths = [];
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        await GitIndexManager.acquire(
          { fs: fs3, gitdir: updatedGitdir, cache },
          async function(index2) {
            unmergedPaths = index2.unmergedPaths;
          }
        );
        const results = await _walk({
          fs: fs3,
          cache,
          dir,
          gitdir: updatedGitdir,
          trees,
          map: async function(path3, [head, workdir, index2]) {
            const staged = !await modified(workdir, index2);
            const unmerged = unmergedPaths.includes(path3);
            const unmodified = !await modified(index2, head);
            if (staged || unmerged) {
              return head ? {
                path: path3,
                mode: await head.mode(),
                oid: await head.oid(),
                type: await head.type(),
                content: await head.content()
              } : void 0;
            }
            if (unmodified) return false;
            else throw new IndexResetError(path3);
          }
        });
        await GitIndexManager.acquire(
          { fs: fs3, gitdir: updatedGitdir, cache },
          async function(index2) {
            for (const entry of results) {
              if (entry === false) continue;
              if (!entry) {
                await fs3.rmdir(`${dir}/${entry.path}`, { recursive: true });
                index2.delete({ filepath: entry.path });
                continue;
              }
              if (entry.type === "blob") {
                const content = new TextDecoder().decode(entry.content);
                await fs3.write(`${dir}/${entry.path}`, content, {
                  mode: entry.mode
                });
                index2.insert({
                  filepath: entry.path,
                  oid: entry.oid,
                  stage: 0
                });
              }
            }
          }
        );
      } catch (err) {
        err.caller = "git.abortMerge";
        throw err;
      }
    }
    var GitIgnoreManager = class {
      /**
       * Determines whether a given file is ignored based on `.gitignore` rules and exclusion files.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} args.dir - The working directory.
       * @param {string} [args.gitdir=join(dir, '.git')] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {string} args.filepath - The path of the file to check.
       * @returns {Promise<boolean>} - `true` if the file is ignored, `false` otherwise.
       */
      static async isIgnored({ fs: fs3, dir, gitdir = join2(dir, ".git"), filepath }) {
        if (basename(filepath) === ".git") return true;
        if (filepath === ".") return false;
        let excludes = "";
        const excludesFile = join2(gitdir, "info", "exclude");
        if (await fs3.exists(excludesFile)) {
          excludes = await fs3.read(excludesFile, "utf8");
        }
        const pairs = [
          {
            gitignore: join2(dir, ".gitignore"),
            filepath
          }
        ];
        const pieces = filepath.split("/").filter(Boolean);
        for (let i = 1; i < pieces.length; i++) {
          const folder = pieces.slice(0, i).join("/");
          const file = pieces.slice(i).join("/");
          pairs.push({
            gitignore: join2(dir, folder, ".gitignore"),
            filepath: file
          });
        }
        let ignoredStatus = false;
        for (const p of pairs) {
          let file;
          try {
            file = await fs3.read(p.gitignore, "utf8");
          } catch (err) {
            if (err.code === "NOENT") continue;
          }
          const ign = ignore().add(excludes);
          ign.add(file);
          const parentdir = dirname(p.filepath);
          if (parentdir !== "." && ign.ignores(parentdir)) return true;
          if (ignoredStatus) {
            ignoredStatus = !ign.test(p.filepath).unignored;
          } else {
            ignoredStatus = ign.test(p.filepath).ignored;
          }
        }
        return ignoredStatus;
      }
    };
    async function writeObjectLoose({ fs: fs3, gitdir, object, format, oid }) {
      if (format !== "deflated") {
        throw new InternalError(
          "GitObjectStoreLoose expects objects to write to be in deflated format"
        );
      }
      const source = `objects/${oid.slice(0, 2)}/${oid.slice(2)}`;
      const filepath = `${gitdir}/${source}`;
      if (!await fs3.exists(filepath)) await fs3.write(filepath, object);
    }
    var supportsCompressionStream = null;
    async function deflate(buffer) {
      if (supportsCompressionStream === null) {
        supportsCompressionStream = testCompressionStream();
      }
      return supportsCompressionStream ? browserDeflate(buffer) : pako.deflate(buffer);
    }
    async function browserDeflate(buffer) {
      const cs = new CompressionStream("deflate");
      const c = new Blob([buffer]).stream().pipeThrough(cs);
      return new Uint8Array(await new Response(c).arrayBuffer());
    }
    function testCompressionStream() {
      try {
        const cs = new CompressionStream("deflate");
        cs.writable.close();
        const stream = new Blob([]).stream();
        stream.cancel();
        return true;
      } catch (_) {
        return false;
      }
    }
    async function _writeObject({
      fs: fs3,
      gitdir,
      type,
      object,
      format = "content",
      oid = void 0,
      dryRun = false
    }) {
      if (format !== "deflated") {
        if (format !== "wrapped") {
          object = GitObject.wrap({ type, object });
        }
        oid = await shasum(object);
        object = Buffer.from(await deflate(object));
      }
      if (!dryRun) {
        await writeObjectLoose({ fs: fs3, gitdir, object, format: "deflated", oid });
      }
      return oid;
    }
    function posixifyPathBuffer(buffer) {
      let idx;
      while (~(idx = buffer.indexOf(92))) buffer[idx] = 47;
      return buffer;
    }
    async function add({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      filepath,
      cache = {},
      force = false,
      parallel = true
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("dir", dir);
        assertParameter("gitdir", gitdir);
        assertParameter("filepath", filepath);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        await GitIndexManager.acquire(
          { fs: fs3, gitdir: updatedGitdir, cache },
          async (index2) => {
            const config = await GitConfigManager.get({ fs: fs3, gitdir: updatedGitdir });
            const autocrlf = await config.get("core.autocrlf");
            return addToIndex({
              dir,
              gitdir: updatedGitdir,
              fs: fs3,
              filepath,
              index: index2,
              force,
              parallel,
              autocrlf
            });
          }
        );
      } catch (err) {
        err.caller = "git.add";
        throw err;
      }
    }
    async function addToIndex({
      dir,
      gitdir,
      fs: fs3,
      filepath,
      index: index2,
      force,
      parallel,
      autocrlf
    }) {
      filepath = Array.isArray(filepath) ? filepath : [filepath];
      const promises = filepath.map(async (currentFilepath) => {
        if (!force) {
          const ignored = await GitIgnoreManager.isIgnored({
            fs: fs3,
            dir,
            gitdir,
            filepath: currentFilepath
          });
          if (ignored) return;
        }
        const stats = await fs3.lstat(join2(dir, currentFilepath));
        if (!stats) throw new NotFoundError(currentFilepath);
        if (stats.isDirectory()) {
          const children = await fs3.readdir(join2(dir, currentFilepath));
          if (parallel) {
            const promises2 = children.map(
              (child) => addToIndex({
                dir,
                gitdir,
                fs: fs3,
                filepath: [join2(currentFilepath, child)],
                index: index2,
                force,
                parallel,
                autocrlf
              })
            );
            await Promise.all(promises2);
          } else {
            for (const child of children) {
              await addToIndex({
                dir,
                gitdir,
                fs: fs3,
                filepath: [join2(currentFilepath, child)],
                index: index2,
                force,
                parallel,
                autocrlf
              });
            }
          }
        } else {
          const object = stats.isSymbolicLink() ? await fs3.readlink(join2(dir, currentFilepath)).then(posixifyPathBuffer) : await fs3.read(join2(dir, currentFilepath), { autocrlf });
          if (object === null) throw new NotFoundError(currentFilepath);
          const oid = await _writeObject({ fs: fs3, gitdir, type: "blob", object });
          index2.insert({ filepath: currentFilepath, stats, oid });
        }
      });
      const settledPromises = await Promise.allSettled(promises);
      const rejectedPromises = settledPromises.filter((settle) => settle.status === "rejected").map((settle) => settle.reason);
      if (rejectedPromises.length > 1) {
        throw new MultipleGitError(rejectedPromises);
      }
      if (rejectedPromises.length === 1) {
        throw rejectedPromises[0];
      }
      const fulfilledPromises = settledPromises.filter((settle) => settle.status === "fulfilled" && settle.value).map((settle) => settle.value);
      return fulfilledPromises;
    }
    async function _getConfig({ fs: fs3, gitdir, path: path3 }) {
      const config = await GitConfigManager.get({ fs: fs3, gitdir });
      return config.get(path3);
    }
    function assignDefined(target, ...sources) {
      for (const source of sources) {
        if (source) {
          for (const key of Object.keys(source)) {
            const val = source[key];
            if (val !== void 0) {
              target[key] = val;
            }
          }
        }
      }
      return target;
    }
    async function normalizeAuthorObject({ fs: fs3, gitdir, author, commit: commit2 }) {
      const timestamp = Math.floor(Date.now() / 1e3);
      const defaultAuthor = {
        name: await _getConfig({ fs: fs3, gitdir, path: "user.name" }),
        email: await _getConfig({ fs: fs3, gitdir, path: "user.email" }) || "",
        // author.email is allowed to be empty string
        timestamp,
        timezoneOffset: new Date(timestamp * 1e3).getTimezoneOffset()
      };
      const normalizedAuthor = assignDefined(
        {},
        defaultAuthor,
        commit2 ? commit2.author : void 0,
        author
      );
      if (normalizedAuthor.name === void 0) {
        return void 0;
      }
      return normalizedAuthor;
    }
    async function normalizeCommitterObject({
      fs: fs3,
      gitdir,
      author,
      committer,
      commit: commit2
    }) {
      const timestamp = Math.floor(Date.now() / 1e3);
      const defaultCommitter = {
        name: await _getConfig({ fs: fs3, gitdir, path: "user.name" }),
        email: await _getConfig({ fs: fs3, gitdir, path: "user.email" }) || "",
        // committer.email is allowed to be empty string
        timestamp,
        timezoneOffset: new Date(timestamp * 1e3).getTimezoneOffset()
      };
      const normalizedCommitter = assignDefined(
        {},
        defaultCommitter,
        commit2 ? commit2.committer : void 0,
        author,
        committer
      );
      if (normalizedCommitter.name === void 0) {
        return void 0;
      }
      return normalizedCommitter;
    }
    async function resolveCommit({ fs: fs3, cache, gitdir, oid }) {
      const { type, object } = await _readObject({ fs: fs3, cache, gitdir, oid });
      if (type === "tag") {
        oid = GitAnnotatedTag.from(object).parse().object;
        return resolveCommit({ fs: fs3, cache, gitdir, oid });
      }
      if (type !== "commit") {
        throw new ObjectTypeError(oid, type, "commit");
      }
      return { commit: GitCommit.from(object), oid };
    }
    async function _readCommit({ fs: fs3, cache, gitdir, oid }) {
      const { commit: commit2, oid: commitOid } = await resolveCommit({
        fs: fs3,
        cache,
        gitdir,
        oid
      });
      const result = {
        oid: commitOid,
        commit: commit2.parse(),
        payload: commit2.withoutSignature()
      };
      return result;
    }
    async function _commit({
      fs: fs3,
      cache,
      onSign,
      gitdir,
      message,
      author: _author,
      committer: _committer,
      signingKey,
      amend = false,
      dryRun = false,
      noUpdateBranch = false,
      ref,
      parent,
      tree
    }) {
      let initialCommit = false;
      let detachedHead = false;
      if (!ref) {
        const headContent = await fs3.read(`${gitdir}/HEAD`, { encoding: "utf8" });
        detachedHead = !headContent.startsWith("ref:");
        ref = await GitRefManager.resolve({
          fs: fs3,
          gitdir,
          ref: "HEAD",
          depth: 2
        });
      }
      let refOid, refCommit;
      try {
        refOid = await GitRefManager.resolve({
          fs: fs3,
          gitdir,
          ref
        });
        refCommit = await _readCommit({ fs: fs3, gitdir, oid: refOid, cache: {} });
      } catch {
        initialCommit = true;
      }
      if (amend && initialCommit) {
        throw new NoCommitError(ref);
      }
      const author = !amend ? await normalizeAuthorObject({ fs: fs3, gitdir, author: _author }) : await normalizeAuthorObject({
        fs: fs3,
        gitdir,
        author: _author,
        commit: refCommit.commit
      });
      if (!author) throw new MissingNameError("author");
      const committer = !amend ? await normalizeCommitterObject({
        fs: fs3,
        gitdir,
        author,
        committer: _committer
      }) : await normalizeCommitterObject({
        fs: fs3,
        gitdir,
        author,
        committer: _committer,
        commit: refCommit.commit
      });
      if (!committer) throw new MissingNameError("committer");
      return GitIndexManager.acquire(
        { fs: fs3, gitdir, cache, allowUnmerged: false },
        async function(index2) {
          const inodes = flatFileListToDirectoryStructure(index2.entries);
          const inode = inodes.get(".");
          if (!tree) {
            tree = await constructTree({ fs: fs3, gitdir, inode, dryRun });
          }
          if (!parent) {
            if (!amend) {
              parent = refOid ? [refOid] : [];
            } else {
              parent = refCommit.commit.parent;
            }
          } else {
            parent = await Promise.all(
              parent.map((p) => {
                return GitRefManager.resolve({ fs: fs3, gitdir, ref: p });
              })
            );
          }
          if (!message) {
            if (!amend) {
              throw new MissingParameterError("message");
            } else {
              message = refCommit.commit.message;
            }
          }
          let comm = GitCommit.from({
            tree,
            parent,
            author,
            committer,
            message
          });
          if (signingKey) {
            comm = await GitCommit.sign(comm, onSign, signingKey);
          }
          const oid = await _writeObject({
            fs: fs3,
            gitdir,
            type: "commit",
            object: comm.toObject(),
            dryRun
          });
          if (!noUpdateBranch && !dryRun) {
            await GitRefManager.writeRef({
              fs: fs3,
              gitdir,
              ref: detachedHead ? "HEAD" : ref,
              value: oid
            });
          }
          return oid;
        }
      );
    }
    async function constructTree({ fs: fs3, gitdir, inode, dryRun }) {
      const children = inode.children;
      for (const inode2 of children) {
        if (inode2.type === "tree") {
          inode2.metadata.mode = "040000";
          inode2.metadata.oid = await constructTree({ fs: fs3, gitdir, inode: inode2, dryRun });
        }
      }
      const entries = children.map((inode2) => ({
        mode: inode2.metadata.mode,
        path: inode2.basename,
        oid: inode2.metadata.oid,
        type: inode2.type
      }));
      const tree = GitTree.from(entries);
      const oid = await _writeObject({
        fs: fs3,
        gitdir,
        type: "tree",
        object: tree.toObject(),
        dryRun
      });
      return oid;
    }
    async function resolveFilepath({ fs: fs3, cache, gitdir, oid, filepath }) {
      if (filepath.startsWith("/")) {
        throw new InvalidFilepathError("leading-slash");
      } else if (filepath.endsWith("/")) {
        throw new InvalidFilepathError("trailing-slash");
      }
      const _oid = oid;
      const result = await resolveTree({ fs: fs3, cache, gitdir, oid });
      const tree = result.tree;
      if (filepath === "") {
        oid = result.oid;
      } else {
        const pathArray = filepath.split("/");
        oid = await _resolveFilepath({
          fs: fs3,
          cache,
          gitdir,
          tree,
          pathArray,
          oid: _oid,
          filepath
        });
      }
      return oid;
    }
    async function _resolveFilepath({
      fs: fs3,
      cache,
      gitdir,
      tree,
      pathArray,
      oid,
      filepath
    }) {
      const name = pathArray.shift();
      for (const entry of tree) {
        if (entry.path === name) {
          if (pathArray.length === 0) {
            return entry.oid;
          } else {
            const { type, object } = await _readObject({
              fs: fs3,
              cache,
              gitdir,
              oid: entry.oid
            });
            if (type !== "tree") {
              throw new ObjectTypeError(oid, type, "tree", filepath);
            }
            tree = GitTree.from(object);
            return _resolveFilepath({
              fs: fs3,
              cache,
              gitdir,
              tree,
              pathArray,
              oid,
              filepath
            });
          }
        }
      }
      throw new NotFoundError(`file or directory found at "${oid}:${filepath}"`);
    }
    async function _readTree({
      fs: fs3,
      cache,
      gitdir,
      oid,
      filepath = void 0
    }) {
      if (filepath !== void 0) {
        oid = await resolveFilepath({ fs: fs3, cache, gitdir, oid, filepath });
      }
      const { tree, oid: treeOid } = await resolveTree({ fs: fs3, cache, gitdir, oid });
      const result = {
        oid: treeOid,
        tree: tree.entries()
      };
      return result;
    }
    async function _writeTree({ fs: fs3, gitdir, tree }) {
      const object = GitTree.from(tree).toObject();
      const oid = await _writeObject({
        fs: fs3,
        gitdir,
        type: "tree",
        object,
        format: "content"
      });
      return oid;
    }
    async function _addNote({
      fs: fs3,
      cache,
      onSign,
      gitdir,
      ref,
      oid,
      note,
      force,
      author,
      committer,
      signingKey
    }) {
      let parent;
      try {
        parent = await GitRefManager.resolve({ gitdir, fs: fs3, ref });
      } catch (err) {
        if (!(err instanceof NotFoundError)) {
          throw err;
        }
      }
      const result = await _readTree({
        fs: fs3,
        cache,
        gitdir,
        oid: parent || "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
      });
      let tree = result.tree;
      if (force) {
        tree = tree.filter((entry) => entry.path !== oid);
      } else {
        for (const entry of tree) {
          if (entry.path === oid) {
            throw new AlreadyExistsError("note", oid);
          }
        }
      }
      if (typeof note === "string") {
        note = Buffer.from(note, "utf8");
      }
      const noteOid = await _writeObject({
        fs: fs3,
        gitdir,
        type: "blob",
        object: note,
        format: "content"
      });
      tree.push({ mode: "100644", path: oid, oid: noteOid, type: "blob" });
      const treeOid = await _writeTree({
        fs: fs3,
        gitdir,
        tree
      });
      const commitOid = await _commit({
        fs: fs3,
        cache,
        onSign,
        gitdir,
        ref,
        tree: treeOid,
        parent: parent && [parent],
        message: `Note added by 'isomorphic-git addNote'
`,
        author,
        committer,
        signingKey
      });
      return commitOid;
    }
    async function addNote({
      fs: _fs,
      onSign,
      dir,
      gitdir = join2(dir, ".git"),
      ref = "refs/notes/commits",
      oid,
      note,
      force,
      author: _author,
      committer: _committer,
      signingKey,
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        assertParameter("note", note);
        if (signingKey) {
          assertParameter("onSign", onSign);
        }
        const fs3 = new FileSystem(_fs);
        const author = await normalizeAuthorObject({ fs: fs3, gitdir, author: _author });
        if (!author) throw new MissingNameError("author");
        const committer = await normalizeCommitterObject({
          fs: fs3,
          gitdir,
          author,
          committer: _committer
        });
        if (!committer) throw new MissingNameError("committer");
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        return await _addNote({
          fs: fs3,
          cache,
          onSign,
          gitdir: updatedGitdir,
          ref,
          oid,
          note,
          force,
          author,
          committer,
          signingKey
        });
      } catch (err) {
        err.caller = "git.addNote";
        throw err;
      }
    }
    var bad = /(^|[/.])([/.]|$)|^@$|@{|[\x00-\x20\x7f~^:?*[\\]|\.lock(\/|$)/;
    function isValidRef(name, onelevel) {
      if (typeof name !== "string")
        throw new TypeError("Reference name must be a string");
      return !bad.test(name) && (!!onelevel || name.includes("/"));
    }
    async function _addRemote({ fs: fs3, gitdir, remote, url, force }) {
      if (!isValidRef(remote, true)) {
        throw new InvalidRefNameError(remote, cleanGitRef.clean(remote));
      }
      const config = await GitConfigManager.get({ fs: fs3, gitdir });
      if (!force) {
        const remoteNames = await config.getSubsections("remote");
        if (remoteNames.includes(remote)) {
          if (url !== await config.get(`remote.${remote}.url`)) {
            throw new AlreadyExistsError("remote", remote);
          }
        }
      }
      await config.set(`remote.${remote}.url`, url);
      await config.set(
        `remote.${remote}.fetch`,
        `+refs/heads/*:refs/remotes/${remote}/*`
      );
      await GitConfigManager.save({ fs: fs3, gitdir, config });
    }
    async function addRemote({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      remote,
      url,
      force = false
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("remote", remote);
        assertParameter("url", url);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _addRemote({
          fs: fsp,
          gitdir: updatedGitdir,
          remote,
          url,
          force
        });
      } catch (err) {
        err.caller = "git.addRemote";
        throw err;
      }
    }
    async function _annotatedTag({
      fs: fs3,
      cache,
      onSign,
      gitdir,
      ref,
      tagger,
      message = ref,
      gpgsig,
      object,
      signingKey,
      force = false
    }) {
      ref = ref.startsWith("refs/tags/") ? ref : `refs/tags/${ref}`;
      if (!force && await GitRefManager.exists({ fs: fs3, gitdir, ref })) {
        throw new AlreadyExistsError("tag", ref);
      }
      const oid = await GitRefManager.resolve({
        fs: fs3,
        gitdir,
        ref: object || "HEAD"
      });
      const { type } = await _readObject({ fs: fs3, cache, gitdir, oid });
      let tagObject = GitAnnotatedTag.from({
        object: oid,
        type,
        tag: ref.replace("refs/tags/", ""),
        tagger,
        message,
        gpgsig
      });
      if (signingKey) {
        tagObject = await GitAnnotatedTag.sign(tagObject, onSign, signingKey);
      }
      const value = await _writeObject({
        fs: fs3,
        gitdir,
        type: "tag",
        object: tagObject.toObject()
      });
      await GitRefManager.writeRef({ fs: fs3, gitdir, ref, value });
    }
    async function annotatedTag({
      fs: _fs,
      onSign,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      tagger: _tagger,
      message = ref,
      gpgsig,
      object,
      signingKey,
      force = false,
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        if (signingKey) {
          assertParameter("onSign", onSign);
        }
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        const tagger = await normalizeAuthorObject({
          fs: fs3,
          gitdir: updatedGitdir,
          author: _tagger
        });
        if (!tagger) throw new MissingNameError("tagger");
        return await _annotatedTag({
          fs: fs3,
          cache,
          onSign,
          gitdir: updatedGitdir,
          ref,
          tagger,
          message,
          gpgsig,
          object,
          signingKey,
          force
        });
      } catch (err) {
        err.caller = "git.annotatedTag";
        throw err;
      }
    }
    async function _branch({
      fs: fs3,
      gitdir,
      ref,
      object,
      checkout: checkout2 = false,
      force = false
    }) {
      if (!isValidRef(ref, true)) {
        throw new InvalidRefNameError(ref, cleanGitRef.clean(ref));
      }
      const fullref = `refs/heads/${ref}`;
      if (!force) {
        const exist = await GitRefManager.exists({ fs: fs3, gitdir, ref: fullref });
        if (exist) {
          throw new AlreadyExistsError("branch", ref, false);
        }
      }
      let oid;
      try {
        oid = await GitRefManager.resolve({ fs: fs3, gitdir, ref: object || "HEAD" });
      } catch (e) {
      }
      if (oid) {
        await GitRefManager.writeRef({ fs: fs3, gitdir, ref: fullref, value: oid });
      }
      if (checkout2) {
        await GitRefManager.writeSymbolicRef({
          fs: fs3,
          gitdir,
          ref: "HEAD",
          value: fullref
        });
      }
    }
    async function branch({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      object,
      checkout: checkout2 = false,
      force = false
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _branch({
          fs: fsp,
          gitdir: updatedGitdir,
          ref,
          object,
          checkout: checkout2,
          force
        });
      } catch (err) {
        err.caller = "git.branch";
        throw err;
      }
    }
    var worthWalking = (filepath, root) => {
      if (filepath === "." || root == null || root.length === 0 || root === ".") {
        return true;
      }
      if (root.length >= filepath.length) {
        return root.startsWith(filepath);
      } else {
        return filepath.startsWith(root);
      }
    };
    async function _checkout({
      fs: fs3,
      cache,
      onProgress,
      onPostCheckout,
      dir,
      gitdir,
      remote,
      ref,
      filepaths,
      noCheckout,
      noUpdateHead,
      dryRun,
      force,
      track = true,
      nonBlocking = false,
      batchSize = 100
    }) {
      let oldOid;
      if (onPostCheckout) {
        try {
          oldOid = await GitRefManager.resolve({ fs: fs3, gitdir, ref: "HEAD" });
        } catch (err) {
          oldOid = "0000000000000000000000000000000000000000";
        }
      }
      let oid;
      try {
        oid = await GitRefManager.resolve({ fs: fs3, gitdir, ref });
      } catch (err) {
        if (ref === "HEAD") throw err;
        const remoteRef = `${remote}/${ref}`;
        oid = await GitRefManager.resolve({
          fs: fs3,
          gitdir,
          ref: remoteRef
        });
        if (track) {
          const config = await GitConfigManager.get({ fs: fs3, gitdir });
          await config.set(`branch.${ref}.remote`, remote);
          await config.set(`branch.${ref}.merge`, `refs/heads/${ref}`);
          await GitConfigManager.save({ fs: fs3, gitdir, config });
        }
        await GitRefManager.writeRef({
          fs: fs3,
          gitdir,
          ref: `refs/heads/${ref}`,
          value: oid
        });
      }
      if (!noCheckout) {
        let ops;
        try {
          ops = await analyze({
            fs: fs3,
            cache,
            onProgress,
            dir,
            gitdir,
            ref,
            force,
            filepaths
          });
        } catch (err) {
          if (err instanceof NotFoundError && err.data.what === oid) {
            throw new CommitNotFetchedError(ref, oid);
          } else {
            throw err;
          }
        }
        const conflicts = ops.filter(([method]) => method === "conflict").map(([method, fullpath]) => fullpath);
        if (conflicts.length > 0) {
          throw new CheckoutConflictError(conflicts);
        }
        const errors = ops.filter(([method]) => method === "error").map(([method, fullpath]) => fullpath);
        if (errors.length > 0) {
          throw new InternalError(errors.join(", "));
        }
        if (dryRun) {
          if (onPostCheckout) {
            await onPostCheckout({
              previousHead: oldOid,
              newHead: oid,
              type: filepaths != null && filepaths.length > 0 ? "file" : "branch"
            });
          }
          return;
        }
        let count = 0;
        const total = ops.length;
        await GitIndexManager.acquire(
          { fs: fs3, gitdir, cache },
          async function(index2) {
            await Promise.all(
              ops.filter(
                ([method]) => method === "delete" || method === "delete-index"
              ).map(async function([method, fullpath]) {
                const filepath = `${dir}/${fullpath}`;
                if (method === "delete") {
                  await fs3.rm(filepath);
                }
                index2.delete({ filepath: fullpath });
                if (onProgress) {
                  await onProgress({
                    phase: "Updating workdir",
                    loaded: ++count,
                    total
                  });
                }
              })
            );
          }
        );
        await GitIndexManager.acquire(
          { fs: fs3, gitdir, cache },
          async function(index2) {
            for (const [method, fullpath] of ops) {
              if (method === "rmdir" || method === "rmdir-index") {
                const filepath = `${dir}/${fullpath}`;
                try {
                  if (method === "rmdir") {
                    await fs3.rmdir(filepath);
                  }
                  index2.delete({ filepath: fullpath });
                  if (onProgress) {
                    await onProgress({
                      phase: "Updating workdir",
                      loaded: ++count,
                      total
                    });
                  }
                } catch (e) {
                  if (e.code === "ENOTEMPTY") {
                    console.log(
                      `Did not delete ${fullpath} because directory is not empty`
                    );
                  } else {
                    throw e;
                  }
                }
              }
            }
          }
        );
        await Promise.all(
          ops.filter(([method]) => method === "mkdir" || method === "mkdir-index").map(async function([_, fullpath]) {
            const filepath = `${dir}/${fullpath}`;
            await fs3.mkdir(filepath);
            if (onProgress) {
              await onProgress({
                phase: "Updating workdir",
                loaded: ++count,
                total
              });
            }
          })
        );
        if (nonBlocking) {
          const eligibleOps = ops.filter(
            ([method]) => method === "create" || method === "create-index" || method === "update" || method === "mkdir-index"
          );
          const updateWorkingDirResults = await batchAllSettled(
            "Update Working Dir",
            eligibleOps.map(
              ([method, fullpath, oid2, mode, chmod]) => () => updateWorkingDir({ fs: fs3, cache, gitdir, dir }, [
                method,
                fullpath,
                oid2,
                mode,
                chmod
              ])
            ),
            onProgress,
            batchSize
          );
          await GitIndexManager.acquire(
            { fs: fs3, gitdir, cache, allowUnmerged: true },
            async function(index2) {
              await batchAllSettled(
                "Update Index",
                updateWorkingDirResults.map(
                  ([fullpath, oid2, stats]) => () => updateIndex({ index: index2, fullpath, oid: oid2, stats })
                ),
                onProgress,
                batchSize
              );
            }
          );
        } else {
          await GitIndexManager.acquire(
            { fs: fs3, gitdir, cache, allowUnmerged: true },
            async function(index2) {
              const settled = await Promise.allSettled(
                ops.filter(
                  ([method]) => method === "create" || method === "create-index" || method === "update" || method === "mkdir-index"
                ).map(async function([method, fullpath, oid2, mode, chmod]) {
                  const filepath = `${dir}/${fullpath}`;
                  if (method !== "create-index" && method !== "mkdir-index") {
                    const { object } = await _readObject({
                      fs: fs3,
                      cache,
                      gitdir,
                      oid: oid2
                    });
                    if (chmod) {
                      await fs3.rm(filepath);
                    }
                    if (mode === 33188) {
                      await fs3.write(filepath, object);
                    } else if (mode === 33261) {
                      await fs3.write(filepath, object, { mode: 511 });
                    } else if (mode === 40960) {
                      await fs3.writelink(filepath, object);
                    } else {
                      throw new InternalError(
                        `Invalid mode 0o${mode.toString(
                          8
                        )} detected in blob ${oid2}`
                      );
                    }
                  }
                  const stats = await fs3.lstat(filepath);
                  if (mode === 33261) {
                    stats.mode = 493;
                  }
                  if (method === "mkdir-index") {
                    stats.mode = 57344;
                  }
                  index2.insert({
                    filepath: fullpath,
                    stats,
                    oid: oid2
                  });
                  if (onProgress) {
                    await onProgress({
                      phase: "Updating workdir",
                      loaded: ++count,
                      total
                    });
                  }
                })
              );
              const rejections = [];
              for (const result of settled) {
                if (result.status === "rejected") {
                  rejections.push(result.reason);
                  console.error(
                    "[isomorphic-git checkout] task rejected:",
                    result.reason?.stack ?? result.reason
                  );
                }
              }
              if (rejections.length > 0) {
                throw new MultipleGitError(rejections);
              }
            }
          );
        }
        if (onPostCheckout) {
          await onPostCheckout({
            previousHead: oldOid,
            newHead: oid,
            type: filepaths != null && filepaths.length > 0 ? "file" : "branch"
          });
        }
      }
      if (!noUpdateHead) {
        const fullRef = await GitRefManager.expand({ fs: fs3, gitdir, ref });
        if (fullRef.startsWith("refs/heads")) {
          await GitRefManager.writeSymbolicRef({
            fs: fs3,
            gitdir,
            ref: "HEAD",
            value: fullRef
          });
        } else {
          await GitRefManager.writeRef({ fs: fs3, gitdir, ref: "HEAD", value: oid });
        }
      }
    }
    async function analyze({
      fs: fs3,
      cache,
      onProgress,
      dir,
      gitdir,
      ref,
      force,
      filepaths
    }) {
      let count = 0;
      return _walk({
        fs: fs3,
        cache,
        dir,
        gitdir,
        trees: [TREE({ ref }), WORKDIR(), STAGE()],
        map: async function(fullpath, [commit2, workdir, stage]) {
          if (fullpath === ".") return;
          if (filepaths && !filepaths.some((base) => worthWalking(fullpath, base))) {
            return null;
          }
          if (onProgress) {
            await onProgress({ phase: "Analyzing workdir", loaded: ++count });
          }
          const key = [!!stage, !!commit2, !!workdir].map(Number).join("");
          switch (key) {
            // Impossible case.
            case "000":
              return;
            // Ignore workdir files that are not tracked and not part of the new commit.
            case "001":
              if (force && filepaths && filepaths.includes(fullpath)) {
                return ["delete", fullpath];
              }
              return;
            // New entries
            case "010": {
              switch (await commit2.type()) {
                case "tree": {
                  return ["mkdir", fullpath];
                }
                case "blob": {
                  return [
                    "create",
                    fullpath,
                    await commit2.oid(),
                    await commit2.mode()
                  ];
                }
                case "commit": {
                  return [
                    "mkdir-index",
                    fullpath,
                    await commit2.oid(),
                    await commit2.mode()
                  ];
                }
                default: {
                  return [
                    "error",
                    `new entry Unhandled type ${await commit2.type()}`
                  ];
                }
              }
            }
            // New entries but there is already something in the workdir there.
            case "011": {
              switch (`${await commit2.type()}-${await workdir.type()}`) {
                case "tree-tree": {
                  return;
                }
                case "tree-blob":
                case "blob-tree": {
                  return ["conflict", fullpath];
                }
                case "blob-blob": {
                  if (await commit2.oid() !== await workdir.oid()) {
                    if (force) {
                      return [
                        "update",
                        fullpath,
                        await commit2.oid(),
                        await commit2.mode(),
                        await commit2.mode() !== await workdir.mode()
                      ];
                    } else {
                      return ["conflict", fullpath];
                    }
                  } else {
                    if (await commit2.mode() !== await workdir.mode()) {
                      if (force) {
                        return [
                          "update",
                          fullpath,
                          await commit2.oid(),
                          await commit2.mode(),
                          true
                        ];
                      } else {
                        return ["conflict", fullpath];
                      }
                    } else {
                      return [
                        "create-index",
                        fullpath,
                        await commit2.oid(),
                        await commit2.mode()
                      ];
                    }
                  }
                }
                case "commit-tree": {
                  return;
                }
                case "commit-blob": {
                  return ["conflict", fullpath];
                }
                default: {
                  return ["error", `new entry Unhandled type ${commit2.type}`];
                }
              }
            }
            // Something in stage but not in the commit OR the workdir.
            // Note: I verified this behavior against canonical git.
            case "100": {
              return ["delete-index", fullpath];
            }
            // Deleted entries
            // TODO: How to handle if stage type and workdir type mismatch?
            case "101": {
              switch (await stage.type()) {
                case "tree": {
                  return ["rmdir-index", fullpath];
                }
                case "blob": {
                  if (await stage.oid() !== await workdir.oid()) {
                    if (force) {
                      return ["delete", fullpath];
                    } else {
                      return ["conflict", fullpath];
                    }
                  } else {
                    return ["delete", fullpath];
                  }
                }
                case "commit": {
                  return ["rmdir-index", fullpath];
                }
                default: {
                  return [
                    "error",
                    `delete entry Unhandled type ${await stage.type()}`
                  ];
                }
              }
            }
            /* eslint-disable no-fallthrough */
            // File missing from workdir
            case "110":
            // Possibly modified entries
            case "111": {
              switch (`${await stage.type()}-${await commit2.type()}`) {
                case "tree-tree": {
                  return;
                }
                case "blob-blob": {
                  if (await stage.oid() === await commit2.oid() && await stage.mode() === await commit2.mode() && !force) {
                    return;
                  }
                  if (workdir) {
                    if (await workdir.oid() !== await stage.oid() && await workdir.oid() !== await commit2.oid()) {
                      if (force) {
                        return [
                          "update",
                          fullpath,
                          await commit2.oid(),
                          await commit2.mode(),
                          await commit2.mode() !== await workdir.mode()
                        ];
                      } else {
                        return ["conflict", fullpath];
                      }
                    }
                  } else if (force) {
                    return [
                      "update",
                      fullpath,
                      await commit2.oid(),
                      await commit2.mode(),
                      await commit2.mode() !== await stage.mode()
                    ];
                  }
                  if (await commit2.mode() !== await stage.mode()) {
                    return [
                      "update",
                      fullpath,
                      await commit2.oid(),
                      await commit2.mode(),
                      true
                    ];
                  }
                  if (await commit2.oid() !== await stage.oid()) {
                    return [
                      "update",
                      fullpath,
                      await commit2.oid(),
                      await commit2.mode(),
                      false
                    ];
                  } else {
                    return;
                  }
                }
                case "tree-blob": {
                  return ["update-dir-to-blob", fullpath, await commit2.oid()];
                }
                case "blob-tree": {
                  return ["update-blob-to-tree", fullpath];
                }
                case "commit-commit": {
                  return [
                    "mkdir-index",
                    fullpath,
                    await commit2.oid(),
                    await commit2.mode()
                  ];
                }
                default: {
                  return [
                    "error",
                    `update entry Unhandled type ${await stage.type()}-${await commit2.type()}`
                  ];
                }
              }
            }
          }
        },
        // Modify the default flat mapping
        reduce: async function(parent, children) {
          children = flat(children);
          if (!parent) {
            return children;
          } else if (parent && parent[0] === "rmdir") {
            children.push(parent);
            return children;
          } else {
            children.unshift(parent);
            return children;
          }
        }
      });
    }
    async function updateIndex({ index: index2, fullpath, stats, oid }) {
      try {
        index2.insert({
          filepath: fullpath,
          stats,
          oid
        });
      } catch (e) {
        console.warn(`Error inserting ${fullpath} into index:`, e);
      }
    }
    async function updateWorkingDir({ fs: fs3, cache, gitdir, dir }, [method, fullpath, oid, mode, chmod]) {
      const filepath = `${dir}/${fullpath}`;
      if (method !== "create-index" && method !== "mkdir-index") {
        const { object } = await _readObject({ fs: fs3, cache, gitdir, oid });
        if (chmod) {
          await fs3.rm(filepath);
        }
        if (mode === 33188) {
          await fs3.write(filepath, object);
        } else if (mode === 33261) {
          await fs3.write(filepath, object, { mode: 511 });
        } else if (mode === 40960) {
          await fs3.writelink(filepath, object);
        } else {
          throw new InternalError(
            `Invalid mode 0o${mode.toString(8)} detected in blob ${oid}`
          );
        }
      }
      const stats = await fs3.lstat(filepath);
      if (mode === 33261) {
        stats.mode = 493;
      }
      if (method === "mkdir-index") {
        stats.mode = 57344;
      }
      return [fullpath, oid, stats];
    }
    async function batchAllSettled(operationName, tasks, onProgress, batchSize) {
      const results = [];
      const rejections = [];
      for (let i = 0; i < tasks.length; i += batchSize) {
        const batch = tasks.slice(i, i + batchSize).map((task) => task());
        const batchResults = await Promise.allSettled(batch);
        batchResults.forEach((result) => {
          if (result.status === "fulfilled") {
            results.push(result.value);
          } else {
            rejections.push(result.reason);
            console.error(
              `[isomorphic-git ${operationName}] task rejected:`,
              result.reason?.stack ?? result.reason
            );
          }
        });
        if (onProgress) {
          await onProgress({
            phase: "Updating workdir",
            loaded: i + batch.length,
            total: tasks.length
          });
        }
      }
      if (rejections.length > 0) {
        throw new MultipleGitError(rejections);
      }
      return results;
    }
    async function checkout({
      fs: fs3,
      onProgress,
      onPostCheckout,
      dir,
      gitdir = join2(dir, ".git"),
      remote = "origin",
      ref: _ref,
      filepaths,
      noCheckout = false,
      noUpdateHead = _ref === void 0,
      dryRun = false,
      force = false,
      track = true,
      cache = {},
      nonBlocking = false,
      batchSize = 100
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("dir", dir);
        assertParameter("gitdir", gitdir);
        const ref = _ref || "HEAD";
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _checkout({
          fs: fsp,
          cache,
          onProgress,
          onPostCheckout,
          dir,
          gitdir: updatedGitdir,
          remote,
          ref,
          filepaths,
          noCheckout,
          noUpdateHead,
          dryRun,
          force,
          track,
          nonBlocking,
          batchSize
        });
      } catch (err) {
        err.caller = "git.checkout";
        throw err;
      }
    }
    var LINEBREAKS = /^.*(\r?\n|$)/gm;
    function mergeFile({ branches, contents }) {
      const ourName = branches[1];
      const theirName = branches[2];
      const baseContent = contents[0];
      const ourContent = contents[1];
      const theirContent = contents[2];
      const ours = ourContent.match(LINEBREAKS);
      const base = baseContent.match(LINEBREAKS);
      const theirs = theirContent.match(LINEBREAKS);
      const result = diff3Merge2(ours, base, theirs);
      const markerSize = 7;
      let mergedText = "";
      let cleanMerge = true;
      for (const item of result) {
        if (item.ok) {
          mergedText += item.ok.join("");
        }
        if (item.conflict) {
          cleanMerge = false;
          mergedText += `${"<".repeat(markerSize)} ${ourName}
`;
          mergedText += item.conflict.a.join("");
          mergedText += `${"=".repeat(markerSize)}
`;
          mergedText += item.conflict.b.join("");
          mergedText += `${">".repeat(markerSize)} ${theirName}
`;
        }
      }
      return { cleanMerge, mergedText };
    }
    async function mergeTree({
      fs: fs3,
      cache,
      dir,
      gitdir = join2(dir, ".git"),
      index: index2,
      ourOid,
      baseOid,
      theirOid,
      ourName = "ours",
      baseName = "base",
      theirName = "theirs",
      dryRun = false,
      abortOnConflict = true,
      mergeDriver
    }) {
      const ourTree = TREE({ ref: ourOid });
      const baseTree = TREE({ ref: baseOid });
      const theirTree = TREE({ ref: theirOid });
      const unmergedFiles = [];
      const bothModified = [];
      const deleteByUs = [];
      const deleteByTheirs = [];
      const results = await _walk({
        fs: fs3,
        cache,
        dir,
        gitdir,
        trees: [ourTree, baseTree, theirTree],
        map: async function(filepath, [ours, base, theirs]) {
          const path3 = basename(filepath);
          const ourChange = await modified(ours, base);
          const theirChange = await modified(theirs, base);
          switch (`${ourChange}-${theirChange}`) {
            case "false-false": {
              return {
                mode: await base.mode(),
                path: path3,
                oid: await base.oid(),
                type: await base.type()
              };
            }
            case "false-true": {
              if (!theirs && await ours.type() === "tree") {
                return {
                  mode: await ours.mode(),
                  path: path3,
                  oid: await ours.oid(),
                  type: await ours.type()
                };
              }
              return theirs ? {
                mode: await theirs.mode(),
                path: path3,
                oid: await theirs.oid(),
                type: await theirs.type()
              } : void 0;
            }
            case "true-false": {
              if (!ours && await theirs.type() === "tree") {
                return {
                  mode: await theirs.mode(),
                  path: path3,
                  oid: await theirs.oid(),
                  type: await theirs.type()
                };
              }
              return ours ? {
                mode: await ours.mode(),
                path: path3,
                oid: await ours.oid(),
                type: await ours.type()
              } : void 0;
            }
            case "true-true": {
              if (ours && theirs && await ours.type() === "tree" && await theirs.type() === "tree") {
                return {
                  mode: await ours.mode(),
                  path: path3,
                  oid: await ours.oid(),
                  type: "tree"
                };
              }
              if (ours && theirs && await ours.type() === "blob" && await theirs.type() === "blob") {
                return mergeBlobs({
                  fs: fs3,
                  gitdir,
                  path: path3,
                  ours,
                  base,
                  theirs,
                  ourName,
                  baseName,
                  theirName,
                  mergeDriver
                }).then(async (r) => {
                  if (!r.cleanMerge) {
                    unmergedFiles.push(filepath);
                    bothModified.push(filepath);
                    if (!abortOnConflict) {
                      let baseOid2 = "";
                      if (base && await base.type() === "blob") {
                        baseOid2 = await base.oid();
                      }
                      const ourOid2 = await ours.oid();
                      const theirOid2 = await theirs.oid();
                      index2.delete({ filepath });
                      if (baseOid2) {
                        index2.insert({ filepath, oid: baseOid2, stage: 1 });
                      }
                      index2.insert({ filepath, oid: ourOid2, stage: 2 });
                      index2.insert({ filepath, oid: theirOid2, stage: 3 });
                    }
                  } else if (!abortOnConflict) {
                    index2.insert({ filepath, oid: r.mergeResult.oid, stage: 0 });
                  }
                  return r.mergeResult;
                });
              }
              if (base && !ours && theirs && await base.type() === "blob" && await theirs.type() === "blob") {
                unmergedFiles.push(filepath);
                deleteByUs.push(filepath);
                if (!abortOnConflict) {
                  const baseOid2 = await base.oid();
                  const theirOid2 = await theirs.oid();
                  index2.delete({ filepath });
                  index2.insert({ filepath, oid: baseOid2, stage: 1 });
                  index2.insert({ filepath, oid: theirOid2, stage: 3 });
                }
                return {
                  mode: await theirs.mode(),
                  oid: await theirs.oid(),
                  type: "blob",
                  path: path3
                };
              }
              if (base && ours && !theirs && await base.type() === "blob" && await ours.type() === "blob") {
                unmergedFiles.push(filepath);
                deleteByTheirs.push(filepath);
                if (!abortOnConflict) {
                  const baseOid2 = await base.oid();
                  const ourOid2 = await ours.oid();
                  index2.delete({ filepath });
                  index2.insert({ filepath, oid: baseOid2, stage: 1 });
                  index2.insert({ filepath, oid: ourOid2, stage: 2 });
                }
                return {
                  mode: await ours.mode(),
                  oid: await ours.oid(),
                  type: "blob",
                  path: path3
                };
              }
              if (base && !ours && !theirs && (await base.type() === "blob" || await base.type() === "tree")) {
                return void 0;
              }
              throw new MergeNotSupportedError();
            }
          }
        },
        /**
         * @param {TreeEntry} [parent]
         * @param {Array<TreeEntry>} children
         */
        reduce: unmergedFiles.length !== 0 && (!dir || abortOnConflict) ? void 0 : async (parent, children) => {
          const entries = children.filter(Boolean);
          if (!parent) return;
          if (parent && parent.type === "tree" && entries.length === 0 && parent.path !== ".")
            return;
          if (entries.length > 0 || parent.path === "." && entries.length === 0) {
            const tree = new GitTree(entries);
            const object = tree.toObject();
            const oid = await _writeObject({
              fs: fs3,
              gitdir,
              type: "tree",
              object,
              dryRun
            });
            parent.oid = oid;
          }
          return parent;
        }
      });
      if (unmergedFiles.length !== 0) {
        if (dir && !abortOnConflict) {
          await _walk({
            fs: fs3,
            cache,
            dir,
            gitdir,
            trees: [TREE({ ref: results.oid })],
            map: async function(filepath, [entry]) {
              const path3 = `${dir}/${filepath}`;
              if (await entry.type() === "blob") {
                const mode = await entry.mode();
                const content = new TextDecoder().decode(await entry.content());
                await fs3.write(path3, content, { mode });
              }
              return true;
            }
          });
        }
        return new MergeConflictError(
          unmergedFiles,
          bothModified,
          deleteByUs,
          deleteByTheirs
        );
      }
      return results.oid;
    }
    async function mergeBlobs({
      fs: fs3,
      gitdir,
      path: path3,
      ours,
      base,
      theirs,
      ourName,
      theirName,
      baseName,
      dryRun,
      mergeDriver = mergeFile
    }) {
      const type = "blob";
      let baseMode = "100755";
      let baseOid = "";
      let baseContent = "";
      if (base && await base.type() === "blob") {
        baseMode = await base.mode();
        baseOid = await base.oid();
        baseContent = Buffer.from(await base.content()).toString("utf8");
      }
      const mode = baseMode === await ours.mode() ? await theirs.mode() : await ours.mode();
      if (await ours.oid() === await theirs.oid()) {
        return {
          cleanMerge: true,
          mergeResult: { mode, path: path3, oid: await ours.oid(), type }
        };
      }
      if (await ours.oid() === baseOid) {
        return {
          cleanMerge: true,
          mergeResult: { mode, path: path3, oid: await theirs.oid(), type }
        };
      }
      if (await theirs.oid() === baseOid) {
        return {
          cleanMerge: true,
          mergeResult: { mode, path: path3, oid: await ours.oid(), type }
        };
      }
      const ourContent = Buffer.from(await ours.content()).toString("utf8");
      const theirContent = Buffer.from(await theirs.content()).toString("utf8");
      const { mergedText, cleanMerge } = await mergeDriver({
        branches: [baseName, ourName, theirName],
        contents: [baseContent, ourContent, theirContent],
        path: path3
      });
      const oid = await _writeObject({
        fs: fs3,
        gitdir,
        type: "blob",
        object: Buffer.from(mergedText, "utf8"),
        dryRun
      });
      return { cleanMerge, mergeResult: { mode, path: path3, oid, type } };
    }
    var _TreeMap = {
      stage: STAGE,
      workdir: WORKDIR
    };
    var lock$2;
    async function acquireLock$1(ref, callback) {
      if (lock$2 === void 0) lock$2 = new AsyncLock();
      return lock$2.acquire(ref, callback);
    }
    async function checkAndWriteBlob(fs3, gitdir, dir, filepath, oid = null) {
      const currentFilepath = join2(dir, filepath);
      const stats = await fs3.lstat(currentFilepath);
      if (!stats) throw new NotFoundError(currentFilepath);
      if (stats.isDirectory())
        throw new InternalError(
          `${currentFilepath}: file expected, but found directory`
        );
      const objContent = oid ? await readObjectLoose({ fs: fs3, gitdir, oid }) : void 0;
      let retOid = objContent ? oid : void 0;
      if (!objContent) {
        await acquireLock$1({ fs: fs3, gitdir, currentFilepath }, async () => {
          const object = stats.isSymbolicLink() ? await fs3.readlink(currentFilepath).then(posixifyPathBuffer) : await fs3.read(currentFilepath);
          if (object === null) throw new NotFoundError(currentFilepath);
          retOid = await _writeObject({ fs: fs3, gitdir, type: "blob", object });
        });
      }
      return retOid;
    }
    async function processTreeEntries({ fs: fs3, dir, gitdir, entries }) {
      async function processTreeEntry(entry) {
        if (entry.type === "tree") {
          if (!entry.oid) {
            const children = await Promise.all(entry.children.map(processTreeEntry));
            entry.oid = await _writeTree({
              fs: fs3,
              gitdir,
              tree: children
            });
            entry.mode = 16384;
          }
        } else if (entry.type === "blob") {
          entry.oid = await checkAndWriteBlob(
            fs3,
            gitdir,
            dir,
            entry.path,
            entry.oid
          );
          entry.mode = 33188;
        }
        entry.path = entry.path.split("/").pop();
        return entry;
      }
      return Promise.all(entries.map(processTreeEntry));
    }
    async function writeTreeChanges({
      fs: fs3,
      dir,
      gitdir,
      treePair
      // [TREE({ ref: 'HEAD' }), 'STAGE'] would be the equivalent of `git write-tree`
    }) {
      const isStage = treePair[1] === "stage";
      const trees = treePair.map((t) => typeof t === "string" ? _TreeMap[t]() : t);
      const changedEntries = [];
      const map = async (filepath, [head, stage]) => {
        if (filepath === "." || await GitIgnoreManager.isIgnored({ fs: fs3, dir, gitdir, filepath })) {
          return;
        }
        if (stage) {
          if (!head || await head.oid() !== await stage.oid() && await stage.oid() !== void 0) {
            changedEntries.push([head, stage]);
          }
          return {
            mode: await stage.mode(),
            path: filepath,
            oid: await stage.oid(),
            type: await stage.type()
          };
        }
      };
      const reduce = async (parent, children) => {
        children = children.filter(Boolean);
        if (!parent) {
          return children.length > 0 ? children : void 0;
        } else {
          parent.children = children;
          return parent;
        }
      };
      const iterate = async (walk2, children) => {
        const filtered = [];
        for (const child of children) {
          const [head, stage] = child;
          if (isStage) {
            if (stage) {
              if (await fs3.exists(`${dir}/${stage.toString()}`)) {
                filtered.push(child);
              } else {
                changedEntries.push([null, stage]);
              }
            }
          } else if (head) {
            if (!stage) {
              changedEntries.push([head, null]);
            } else {
              filtered.push(child);
            }
          }
        }
        return filtered.length ? Promise.all(filtered.map(walk2)) : [];
      };
      const entries = await _walk({
        fs: fs3,
        cache: {},
        dir,
        gitdir,
        trees,
        map,
        reduce,
        iterate
      });
      if (changedEntries.length === 0 || entries.length === 0) {
        return null;
      }
      const processedEntries = await processTreeEntries({
        fs: fs3,
        dir,
        gitdir,
        entries
      });
      const treeEntries = processedEntries.filter(Boolean).map((entry) => ({
        mode: entry.mode,
        path: entry.path,
        oid: entry.oid,
        type: entry.type
      }));
      return _writeTree({ fs: fs3, gitdir, tree: treeEntries });
    }
    async function applyTreeChanges({
      fs: fs3,
      dir,
      gitdir,
      stashCommit,
      parentCommit,
      wasStaged
    }) {
      const dirRemoved = [];
      const stageUpdated = [];
      const ops = await _walk({
        fs: fs3,
        cache: {},
        dir,
        gitdir,
        trees: [TREE({ ref: parentCommit }), TREE({ ref: stashCommit })],
        map: async (filepath, [parent, stash2]) => {
          if (filepath === "." || await GitIgnoreManager.isIgnored({ fs: fs3, dir, gitdir, filepath })) {
            return;
          }
          const type = stash2 ? await stash2.type() : await parent.type();
          if (type !== "tree" && type !== "blob") {
            return;
          }
          if (!stash2 && parent) {
            const method = type === "tree" ? "rmdir" : "rm";
            if (type === "tree") dirRemoved.push(filepath);
            if (type === "blob" && wasStaged)
              stageUpdated.push({ filepath, oid: await parent.oid() });
            return { method, filepath };
          }
          const oid = await stash2.oid();
          if (!parent || await parent.oid() !== oid) {
            if (type === "tree") {
              return { method: "mkdir", filepath };
            } else {
              if (wasStaged)
                stageUpdated.push({
                  filepath,
                  oid,
                  stats: await fs3.lstat(join2(dir, filepath))
                });
              return {
                method: "write",
                filepath,
                oid
              };
            }
          }
        }
      });
      await acquireLock$1({ fs: fs3, gitdir, dirRemoved, ops }, async () => {
        for (const op of ops) {
          const currentFilepath = join2(dir, op.filepath);
          switch (op.method) {
            case "rmdir":
              await fs3.rmdir(currentFilepath);
              break;
            case "mkdir":
              await fs3.mkdir(currentFilepath);
              break;
            case "rm":
              await fs3.rm(currentFilepath);
              break;
            case "write":
              if (!dirRemoved.some(
                (removedDir) => currentFilepath.startsWith(removedDir)
              )) {
                const { object } = await _readObject({
                  fs: fs3,
                  cache: {},
                  gitdir,
                  oid: op.oid
                });
                if (await fs3.exists(currentFilepath)) {
                  await fs3.rm(currentFilepath);
                }
                await fs3.write(currentFilepath, object);
              }
              break;
          }
        }
      });
      await GitIndexManager.acquire({ fs: fs3, gitdir, cache: {} }, async (index2) => {
        stageUpdated.forEach(({ filepath, stats, oid }) => {
          index2.insert({ filepath, stats, oid });
        });
      });
    }
    async function _cherryPick({
      fs: fs3,
      cache,
      dir,
      gitdir,
      oid,
      dryRun = false,
      noUpdateBranch = false,
      abortOnConflict = true,
      committer,
      mergeDriver
    }) {
      const { commit: cherryCommit, oid: cherryOid } = await _readCommit({
        fs: fs3,
        cache,
        gitdir,
        oid
      });
      if (cherryCommit.parent.length > 1) {
        throw new CherryPickMergeCommitError(cherryOid, cherryCommit.parent.length);
      }
      if (cherryCommit.parent.length === 0) {
        throw new CherryPickRootCommitError(cherryOid);
      }
      const currentOid = await GitRefManager.resolve({
        fs: fs3,
        gitdir,
        ref: "HEAD"
      });
      const { commit: currentCommit } = await _readCommit({
        fs: fs3,
        cache,
        gitdir,
        oid: currentOid
      });
      const cherryParentOid = cherryCommit.parent[0];
      const { commit: cherryParent } = await _readCommit({
        fs: fs3,
        cache,
        gitdir,
        oid: cherryParentOid
      });
      const mergedTreeOid = await GitIndexManager.acquire(
        { fs: fs3, gitdir, cache, allowUnmerged: false },
        async (index2) => {
          return mergeTree({
            fs: fs3,
            cache,
            dir,
            gitdir,
            index: index2,
            ourOid: currentCommit.tree,
            baseOid: cherryParent.tree,
            theirOid: cherryCommit.tree,
            ourName: "HEAD",
            baseName: `parent of ${cherryOid.slice(0, 7)}`,
            theirName: cherryOid.slice(0, 7),
            dryRun,
            abortOnConflict,
            mergeDriver
          });
        }
      );
      if (mergedTreeOid instanceof MergeConflictError) {
        throw mergedTreeOid;
      }
      const newOid = await _commit({
        fs: fs3,
        cache,
        gitdir,
        message: cherryCommit.message,
        tree: mergedTreeOid,
        parent: [currentOid],
        // Single parent: current HEAD
        author: cherryCommit.author,
        // Preserve original author
        committer,
        // New committer
        dryRun,
        noUpdateBranch
      });
      if (dir && !dryRun && !noUpdateBranch) {
        await applyTreeChanges({
          fs: fs3,
          dir,
          gitdir,
          stashCommit: newOid,
          parentCommit: currentOid,
          wasStaged: true
        });
      }
      return newOid;
    }
    async function cherryPick({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      oid,
      cache = {},
      committer,
      dryRun = false,
      noUpdateBranch = false,
      abortOnConflict = true,
      mergeDriver
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        const { commit: cherryCommit } = await _readCommit({
          fs: fs3,
          cache,
          gitdir: updatedGitdir,
          oid
        });
        if (cherryCommit.parent && cherryCommit.parent.length > 1) {
          return await _cherryPick({
            fs: fs3,
            cache,
            dir,
            gitdir: updatedGitdir,
            oid,
            dryRun,
            noUpdateBranch,
            abortOnConflict,
            committer: void 0,
            mergeDriver
          });
        }
        const normalizedCommitter = await normalizeCommitterObject({
          fs: fs3,
          gitdir: updatedGitdir,
          committer
        });
        if (!normalizedCommitter) {
          throw new MissingNameError("committer");
        }
        return await _cherryPick({
          fs: fs3,
          cache,
          dir,
          gitdir: updatedGitdir,
          oid,
          dryRun,
          noUpdateBranch,
          abortOnConflict,
          committer: normalizedCommitter,
          mergeDriver
        });
      } catch (err) {
        err.caller = "git.cherryPick";
        throw err;
      }
    }
    var abbreviateRx = /^refs\/(heads\/|tags\/|remotes\/)?(.*)/;
    function abbreviateRef(ref) {
      const match = abbreviateRx.exec(ref);
      if (match) {
        if (match[1] === "remotes/" && ref.endsWith("/HEAD")) {
          return match[2].slice(0, -5);
        } else {
          return match[2];
        }
      }
      return ref;
    }
    async function _currentBranch({
      fs: fs3,
      gitdir,
      fullname = false,
      test = false
    }) {
      const ref = await GitRefManager.resolve({
        fs: fs3,
        gitdir,
        ref: "HEAD",
        depth: 2
      });
      if (test) {
        try {
          await GitRefManager.resolve({ fs: fs3, gitdir, ref });
        } catch (_) {
          return;
        }
      }
      if (!ref.startsWith("refs/")) return;
      return fullname ? ref : abbreviateRef(ref);
    }
    function translateSSHtoHTTP(url) {
      url = url.replace(/^git@([^:]+):/, "https://$1/");
      url = url.replace(/^ssh:\/\//, "https://");
      return url;
    }
    function calculateBasicAuthHeader({ username = "", password = "" }) {
      return `Basic ${Buffer.from(`${username}:${password}`).toString("base64")}`;
    }
    async function forAwait(iterable, cb) {
      const iter = getIterator(iterable);
      while (true) {
        const { value, done } = await iter.next();
        if (value) await cb(value);
        if (done) break;
      }
      if (iter.return) iter.return();
    }
    async function collect(iterable) {
      let size = 0;
      const buffers = [];
      await forAwait(iterable, (value) => {
        buffers.push(value);
        size += value.byteLength;
      });
      const result = new Uint8Array(size);
      let nextIndex = 0;
      for (const buffer of buffers) {
        result.set(buffer, nextIndex);
        nextIndex += buffer.byteLength;
      }
      return result;
    }
    function extractAuthFromUrl(url) {
      let userpass = url.match(/^https?:\/\/([^/]+)@/);
      if (userpass == null) return { url, auth: {} };
      userpass = userpass[1];
      const [username, password] = userpass.split(":");
      url = url.replace(`${userpass}@`, "");
      return { url, auth: { username, password } };
    }
    function padHex(b, n) {
      const s = n.toString(16);
      return "0".repeat(b - s.length) + s;
    }
    var GitPktLine = class {
      static flush() {
        return Buffer.from("0000", "utf8");
      }
      static delim() {
        return Buffer.from("0001", "utf8");
      }
      static encode(line) {
        if (typeof line === "string") {
          line = Buffer.from(line);
        }
        const length = line.length + 4;
        const hexlength = padHex(4, length);
        return Buffer.concat([Buffer.from(hexlength, "utf8"), line]);
      }
      static streamReader(stream) {
        const reader = new StreamReader(stream);
        return async function read() {
          try {
            let length = await reader.read(4);
            if (length == null) return true;
            length = parseInt(length.toString("utf8"), 16);
            if (length === 0) return null;
            if (length === 1) return null;
            const buffer = await reader.read(length - 4);
            if (buffer == null) return true;
            return buffer;
          } catch (err) {
            stream.error = err;
            return true;
          }
        };
      }
    };
    async function parseCapabilitiesV2(read) {
      const capabilities2 = {};
      let line;
      while (true) {
        line = await read();
        if (line === true) break;
        if (line === null) continue;
        line = line.toString("utf8").replace(/\n$/, "");
        const i = line.indexOf("=");
        if (i > -1) {
          const key = line.slice(0, i);
          const value = line.slice(i + 1);
          capabilities2[key] = value;
        } else {
          capabilities2[line] = true;
        }
      }
      return { protocolVersion: 2, capabilities2 };
    }
    async function parseRefsAdResponse(stream, { service }) {
      const capabilities = /* @__PURE__ */ new Set();
      const refs = /* @__PURE__ */ new Map();
      const symrefs = /* @__PURE__ */ new Map();
      const read = GitPktLine.streamReader(stream);
      let lineOne = await read();
      while (lineOne === null) lineOne = await read();
      if (lineOne === true) throw new EmptyServerResponseError();
      if (lineOne.includes("version 2")) {
        return parseCapabilitiesV2(read);
      }
      if (lineOne.toString("utf8").replace(/\n$/, "") !== `# service=${service}`) {
        throw new ParseError(`# service=${service}\\n`, lineOne.toString("utf8"));
      }
      let lineTwo = await read();
      while (lineTwo === null) lineTwo = await read();
      if (lineTwo === true) return { capabilities, refs, symrefs };
      lineTwo = lineTwo.toString("utf8");
      if (lineTwo.includes("version 2")) {
        return parseCapabilitiesV2(read);
      }
      const [firstRef, capabilitiesLine] = splitAndAssert(lineTwo, "\0", "\\x00");
      capabilitiesLine.split(" ").map((x) => capabilities.add(x));
      if (firstRef !== "0000000000000000000000000000000000000000 capabilities^{}") {
        const [ref, name] = splitAndAssert(firstRef, " ", " ");
        refs.set(name, ref);
        while (true) {
          const line = await read();
          if (line === true) break;
          if (line !== null) {
            const [ref2, name2] = splitAndAssert(line.toString("utf8"), " ", " ");
            refs.set(name2, ref2);
          }
        }
      }
      for (const cap of capabilities) {
        if (cap.startsWith("symref=")) {
          const m = cap.match(/symref=([^:]+):(.*)/);
          if (m.length === 3) {
            symrefs.set(m[1], m[2]);
          }
        }
      }
      return { protocolVersion: 1, capabilities, refs, symrefs };
    }
    function splitAndAssert(line, sep, expected) {
      const split = line.trim().split(sep);
      if (split.length !== 2) {
        throw new ParseError(
          `Two strings separated by '${expected}'`,
          line.toString("utf8")
        );
      }
      return split;
    }
    var corsProxify = (corsProxy, url) => corsProxy.endsWith("?") ? `${corsProxy}${url}` : `${corsProxy}/${url.replace(/^https?:\/\//, "")}`;
    var updateHeaders = (headers, auth) => {
      if (auth.username || auth.password) {
        headers.Authorization = calculateBasicAuthHeader(auth);
      }
      if (auth.headers) {
        Object.assign(headers, auth.headers);
      }
    };
    var stringifyBody = async (res) => {
      try {
        const data = Buffer.from(await collect(res.body));
        const response = data.toString("utf8");
        const preview = response.length < 256 ? response : response.slice(0, 256) + "...";
        return { preview, response, data };
      } catch (e) {
        return {};
      }
    };
    var GitRemoteHTTP = class {
      /**
       * Returns the capabilities of the GitRemoteHTTP class.
       *
       * @returns {Promise<string[]>} - An array of supported capabilities.
       */
      static async capabilities() {
        return ["discover", "connect"];
      }
      /**
       * Discovers references from a remote Git repository.
       *
       * @param {Object} args
       * @param {HttpClient} args.http - The HTTP client to use for requests.
       * @param {ProgressCallback} [args.onProgress] - Callback for progress updates.
       * @param {AuthCallback} [args.onAuth] - Callback for providing authentication credentials.
       * @param {AuthFailureCallback} [args.onAuthFailure] - Callback for handling authentication failures.
       * @param {AuthSuccessCallback} [args.onAuthSuccess] - Callback for handling successful authentication.
       * @param {string} [args.corsProxy] - Optional CORS proxy URL.
       * @param {string} args.service - The Git service (e.g., "git-upload-pack").
       * @param {string} args.url - The URL of the remote repository.
       * @param {Object<string, string>} args.headers - HTTP headers to include in the request.
       * @param {1 | 2} args.protocolVersion - The Git protocol version to use.
       * @returns {Promise<Object>} - The parsed response from the remote repository.
       * @throws {HttpError} - If the HTTP request fails.
       * @throws {SmartHttpError} - If the response cannot be parsed.
       * @throws {UserCanceledError} - If the user cancels the operation.
       */
      static async discover({
        http,
        onProgress,
        onAuth,
        onAuthSuccess,
        onAuthFailure,
        corsProxy,
        service,
        url: _origUrl,
        headers,
        protocolVersion
      }) {
        let { url, auth } = extractAuthFromUrl(_origUrl);
        const proxifiedURL = corsProxy ? corsProxify(corsProxy, url) : url;
        if (auth.username || auth.password) {
          headers.Authorization = calculateBasicAuthHeader(auth);
        }
        if (protocolVersion === 2) {
          headers["Git-Protocol"] = "version=2";
        }
        let res;
        let tryAgain;
        let providedAuthBefore = false;
        do {
          res = await http.request({
            onProgress,
            method: "GET",
            url: `${proxifiedURL}/info/refs?service=${service}`,
            headers
          });
          tryAgain = false;
          if (res.statusCode === 401 || res.statusCode === 203) {
            const getAuth = providedAuthBefore ? onAuthFailure : onAuth;
            if (getAuth) {
              auth = await getAuth(url, {
                ...auth,
                headers: { ...headers }
              });
              if (auth && auth.cancel) {
                throw new UserCanceledError();
              } else if (auth) {
                updateHeaders(headers, auth);
                providedAuthBefore = true;
                tryAgain = true;
              }
            }
          } else if (res.statusCode === 200 && providedAuthBefore && onAuthSuccess) {
            await onAuthSuccess(url, auth);
          }
        } while (tryAgain);
        if (res.statusCode !== 200) {
          const { response } = await stringifyBody(res);
          throw new HttpError(res.statusCode, res.statusMessage, response);
        }
        if (res.headers["content-type"] === `application/x-${service}-advertisement`) {
          const remoteHTTP = await parseRefsAdResponse(res.body, { service });
          remoteHTTP.auth = auth;
          return remoteHTTP;
        } else {
          const { preview, response, data } = await stringifyBody(res);
          try {
            const remoteHTTP = await parseRefsAdResponse([data], { service });
            remoteHTTP.auth = auth;
            return remoteHTTP;
          } catch (e) {
            throw new SmartHttpError(preview, response);
          }
        }
      }
      /**
       * Connects to a remote Git repository and sends a request.
       *
       * @param {Object} args
       * @param {HttpClient} args.http - The HTTP client to use for requests.
       * @param {ProgressCallback} [args.onProgress] - Callback for progress updates.
       * @param {string} [args.corsProxy] - Optional CORS proxy URL.
       * @param {string} args.service - The Git service (e.g., "git-upload-pack").
       * @param {string} args.url - The URL of the remote repository.
       * @param {Object<string, string>} [args.headers] - HTTP headers to include in the request.
       * @param {any} args.body - The request body to send.
       * @param {any} args.auth - Authentication credentials.
       * @returns {Promise<GitHttpResponse>} - The HTTP response from the remote repository.
       * @throws {HttpError} - If the HTTP request fails.
       */
      static async connect({
        http,
        onProgress,
        corsProxy,
        service,
        url,
        auth,
        body,
        headers
      }) {
        const urlAuth = extractAuthFromUrl(url);
        if (urlAuth) url = urlAuth.url;
        if (corsProxy) url = corsProxify(corsProxy, url);
        headers["content-type"] = `application/x-${service}-request`;
        headers.accept = `application/x-${service}-result`;
        updateHeaders(headers, auth);
        const res = await http.request({
          onProgress,
          method: "POST",
          url: `${url}/${service}`,
          body,
          headers
        });
        if (res.statusCode !== 200) {
          const { response } = stringifyBody(res);
          throw new HttpError(res.statusCode, res.statusMessage, response);
        }
        return res;
      }
    };
    var GitRemoteManager = class {
      /**
       * Determines the appropriate remote helper for the given URL.
       *
       * @param {Object} args
       * @param {string} args.url - The URL of the remote repository.
       * @returns {Object} - The remote helper class for the specified transport.
       * @throws {UrlParseError} - If the URL cannot be parsed.
       * @throws {UnknownTransportError} - If the transport is not supported.
       */
      static getRemoteHelperFor({ url }) {
        const remoteHelpers = /* @__PURE__ */ new Map();
        remoteHelpers.set("http", GitRemoteHTTP);
        remoteHelpers.set("https", GitRemoteHTTP);
        const parts = parseRemoteUrl({ url });
        if (!parts) {
          throw new UrlParseError(url);
        }
        if (remoteHelpers.has(parts.transport)) {
          return remoteHelpers.get(parts.transport);
        }
        throw new UnknownTransportError(
          url,
          parts.transport,
          parts.transport === "ssh" ? translateSSHtoHTTP(url) : void 0
        );
      }
    };
    function parseRemoteUrl({ url }) {
      if (url.startsWith("git@")) {
        return {
          transport: "ssh",
          address: url
        };
      }
      const matches = url.match(/(\w+)(:\/\/|::)(.*)/);
      if (matches === null) return;
      if (matches[2] === "://") {
        return {
          transport: matches[1],
          address: matches[0]
        };
      }
      if (matches[2] === "::") {
        return {
          transport: matches[1],
          address: matches[3]
        };
      }
    }
    var lock$3 = null;
    var GitShallowManager = class {
      /**
       * Reads the `shallow` file in the Git repository and returns a set of object IDs (OIDs).
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir] - [required] The [git directory](dir-vs-gitdir.md) path
       * @returns {Promise<Set<string>>} - A set of shallow object IDs.
       */
      static async read({ fs: fs3, gitdir }) {
        if (lock$3 === null) lock$3 = new AsyncLock();
        const filepath = join2(gitdir, "shallow");
        const oids = /* @__PURE__ */ new Set();
        await lock$3.acquire(filepath, async function() {
          const text = await fs3.read(filepath, { encoding: "utf8" });
          if (text === null) return oids;
          if (text.trim() === "") return oids;
          text.trim().split("\n").map((oid) => oids.add(oid));
        });
        return oids;
      }
      /**
       * Writes a set of object IDs (OIDs) to the `shallow` file in the Git repository.
       * If the set is empty, the `shallow` file is removed.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} [args.gitdir] - [required] The [git directory](dir-vs-gitdir.md) path
       * @param {Set<string>} args.oids - A set of shallow object IDs to write.
       * @returns {Promise<void>}
       */
      static async write({ fs: fs3, gitdir, oids }) {
        if (lock$3 === null) lock$3 = new AsyncLock();
        const filepath = join2(gitdir, "shallow");
        if (oids.size > 0) {
          const text = [...oids].join("\n") + "\n";
          await lock$3.acquire(filepath, async function() {
            await fs3.write(filepath, text, {
              encoding: "utf8"
            });
          });
        } else {
          await lock$3.acquire(filepath, async function() {
            await fs3.rm(filepath);
          });
        }
      }
    };
    async function hasObjectLoose({ fs: fs3, gitdir, oid }) {
      const source = `objects/${oid.slice(0, 2)}/${oid.slice(2)}`;
      return fs3.exists(`${gitdir}/${source}`);
    }
    async function hasObjectPacked({
      fs: fs3,
      cache,
      gitdir,
      oid,
      getExternalRefDelta
    }) {
      let list = await fs3.readdir(join2(gitdir, "objects/pack"));
      list = list.filter((x) => x.endsWith(".idx"));
      for (const filename of list) {
        const indexFile = `${gitdir}/objects/pack/${filename}`;
        const p = await readPackIndex({
          fs: fs3,
          cache,
          filename: indexFile,
          getExternalRefDelta
        });
        if (p.error) throw new InternalError(p.error);
        if (p.offsets.has(oid)) {
          return true;
        }
      }
      return false;
    }
    async function hasObject({
      fs: fs3,
      cache,
      gitdir,
      oid,
      format = "content"
    }) {
      const getExternalRefDelta = (oid2) => _readObject({ fs: fs3, cache, gitdir, oid: oid2 });
      let result = await hasObjectLoose({ fs: fs3, gitdir, oid });
      if (!result) {
        result = await hasObjectPacked({
          fs: fs3,
          cache,
          gitdir,
          oid,
          getExternalRefDelta
        });
      }
      return result;
    }
    function emptyPackfile(pack) {
      const pheader = "5041434b";
      const version2 = "00000002";
      const obCount = "00000000";
      const header = pheader + version2 + obCount;
      return pack.slice(0, 12).toString("hex") === header;
    }
    function filterCapabilities(server, client) {
      const serverNames = server.map((cap) => cap.split("=", 1)[0]);
      return client.filter((cap) => {
        const name = cap.split("=", 1)[0];
        return serverNames.includes(name);
      });
    }
    var pkg = {
      name: "isomorphic-git",
      version: "1.37.6",
      agent: "git/isomorphic-git@1.37.6"
    };
    var FIFO = class {
      constructor() {
        this._queue = [];
      }
      write(chunk) {
        if (this._ended) {
          throw Error("You cannot write to a FIFO that has already been ended!");
        }
        if (this._waiting) {
          const resolve = this._waiting;
          this._waiting = null;
          resolve({ value: chunk });
        } else {
          this._queue.push(chunk);
        }
      }
      end() {
        this._ended = true;
        if (this._waiting) {
          const resolve = this._waiting;
          this._waiting = null;
          resolve({ done: true });
        }
      }
      destroy(err) {
        this.error = err;
        this.end();
      }
      async next() {
        if (this._queue.length > 0) {
          return { value: this._queue.shift() };
        }
        if (this._ended) {
          return { done: true };
        }
        if (this._waiting) {
          throw Error(
            "You cannot call read until the previous call to read has returned!"
          );
        }
        return new Promise((resolve) => {
          this._waiting = resolve;
        });
      }
    };
    function findSplit(str) {
      const r = str.indexOf("\r");
      const n = str.indexOf("\n");
      if (r === -1 && n === -1) return -1;
      if (r === -1) return n + 1;
      if (n === -1) return r + 1;
      if (n === r + 1) return n + 1;
      return Math.min(r, n) + 1;
    }
    function splitLines(input2) {
      const output2 = new FIFO();
      let tmp = "";
      (async () => {
        await forAwait(input2, (chunk) => {
          chunk = chunk.toString("utf8");
          tmp += chunk;
          while (true) {
            const i = findSplit(tmp);
            if (i === -1) break;
            output2.write(tmp.slice(0, i));
            tmp = tmp.slice(i);
          }
        });
        if (tmp.length > 0) {
          output2.write(tmp);
        }
        output2.end();
      })();
      return output2;
    }
    var GitSideBand = class {
      static demux(input2) {
        const read = GitPktLine.streamReader(input2);
        const packetlines = new FIFO();
        const packfile = new FIFO();
        const progress = new FIFO();
        const nextBit = async function() {
          const line = await read();
          if (line === null) return nextBit();
          if (line === true) {
            packetlines.end();
            progress.end();
            input2.error ? packfile.destroy(input2.error) : packfile.end();
            return;
          }
          switch (line[0]) {
            case 1: {
              packfile.write(line.slice(1));
              break;
            }
            case 2: {
              progress.write(line.slice(1));
              break;
            }
            case 3: {
              const error = line.slice(1);
              progress.write(error);
              packetlines.end();
              progress.end();
              packfile.destroy(new Error(error.toString("utf8")));
              return;
            }
            default: {
              packetlines.write(line);
            }
          }
          nextBit();
        };
        nextBit();
        return {
          packetlines,
          packfile,
          progress
        };
      }
      // static mux ({
      //   protocol, // 'side-band' or 'side-band-64k'
      //   packetlines,
      //   packfile,
      //   progress,
      //   error
      // }) {
      //   const MAX_PACKET_LENGTH = protocol === 'side-band-64k' ? 999 : 65519
      //   let output = new PassThrough()
      //   packetlines.on('data', data => {
      //     if (data === null) {
      //       output.write(GitPktLine.flush())
      //     } else {
      //       output.write(GitPktLine.encode(data))
      //     }
      //   })
      //   let packfileWasEmpty = true
      //   let packfileEnded = false
      //   let progressEnded = false
      //   let errorEnded = false
      //   let goodbye = Buffer.concat([
      //     GitPktLine.encode(Buffer.from('010A', 'hex')),
      //     GitPktLine.flush()
      //   ])
      //   packfile
      //     .on('data', data => {
      //       packfileWasEmpty = false
      //       const buffers = splitBuffer(data, MAX_PACKET_LENGTH)
      //       for (const buffer of buffers) {
      //         output.write(
      //           GitPktLine.encode(Buffer.concat([Buffer.from('01', 'hex'), buffer]))
      //         )
      //       }
      //     })
      //     .on('end', () => {
      //       packfileEnded = true
      //       if (!packfileWasEmpty) output.write(goodbye)
      //       if (progressEnded && errorEnded) output.end()
      //     })
      //   progress
      //     .on('data', data => {
      //       const buffers = splitBuffer(data, MAX_PACKET_LENGTH)
      //       for (const buffer of buffers) {
      //         output.write(
      //           GitPktLine.encode(Buffer.concat([Buffer.from('02', 'hex'), buffer]))
      //         )
      //       }
      //     })
      //     .on('end', () => {
      //       progressEnded = true
      //       if (packfileEnded && errorEnded) output.end()
      //     })
      //   error
      //     .on('data', data => {
      //       const buffers = splitBuffer(data, MAX_PACKET_LENGTH)
      //       for (const buffer of buffers) {
      //         output.write(
      //           GitPktLine.encode(Buffer.concat([Buffer.from('03', 'hex'), buffer]))
      //         )
      //       }
      //     })
      //     .on('end', () => {
      //       errorEnded = true
      //       if (progressEnded && packfileEnded) output.end()
      //     })
      //   return output
      // }
    };
    async function parseUploadPackResponse(stream) {
      const { packetlines, packfile, progress } = GitSideBand.demux(stream);
      const shallows = [];
      const unshallows = [];
      const acks = [];
      let nak = false;
      let done = false;
      return new Promise((resolve, reject) => {
        forAwait(packetlines, (data) => {
          const line = data.toString("utf8").trim();
          if (line.startsWith("shallow")) {
            const oid = line.slice(-41).trim();
            if (oid.length !== 40) {
              reject(new InvalidOidError(oid));
            }
            shallows.push(oid);
          } else if (line.startsWith("unshallow")) {
            const oid = line.slice(-41).trim();
            if (oid.length !== 40) {
              reject(new InvalidOidError(oid));
            }
            unshallows.push(oid);
          } else if (line.startsWith("ACK")) {
            const [, oid, status2] = line.split(" ");
            acks.push({ oid, status: status2 });
            if (!status2) done = true;
          } else if (line.startsWith("NAK")) {
            nak = true;
            done = true;
          } else {
            done = true;
            nak = true;
          }
          if (done) {
            stream.error ? reject(stream.error) : resolve({ shallows, unshallows, acks, nak, packfile, progress });
          }
        }).finally(() => {
          if (!done) {
            stream.error ? reject(stream.error) : resolve({ shallows, unshallows, acks, nak, packfile, progress });
          }
        });
      });
    }
    function writeUploadPackRequest({
      capabilities = [],
      wants = [],
      haves = [],
      shallows = [],
      depth = null,
      since = null,
      exclude = []
    }) {
      const packstream = [];
      wants = [...new Set(wants)];
      let firstLineCapabilities = ` ${capabilities.join(" ")}`;
      for (const oid of wants) {
        packstream.push(GitPktLine.encode(`want ${oid}${firstLineCapabilities}
`));
        firstLineCapabilities = "";
      }
      for (const oid of shallows) {
        packstream.push(GitPktLine.encode(`shallow ${oid}
`));
      }
      if (depth !== null) {
        packstream.push(GitPktLine.encode(`deepen ${depth}
`));
      }
      if (since !== null) {
        packstream.push(
          GitPktLine.encode(`deepen-since ${Math.floor(since.valueOf() / 1e3)}
`)
        );
      }
      for (const oid of exclude) {
        packstream.push(GitPktLine.encode(`deepen-not ${oid}
`));
      }
      packstream.push(GitPktLine.flush());
      for (const oid of haves) {
        packstream.push(GitPktLine.encode(`have ${oid}
`));
      }
      packstream.push(GitPktLine.encode(`done
`));
      return packstream;
    }
    async function _fetch({
      fs: fs3,
      cache,
      http,
      onProgress,
      onMessage,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      gitdir,
      ref: _ref,
      remoteRef: _remoteRef,
      remote: _remote,
      url: _url,
      corsProxy,
      depth = null,
      since = null,
      exclude = [],
      relative = false,
      tags = false,
      singleBranch = false,
      headers = {},
      prune = false,
      pruneTags = false
    }) {
      const ref = _ref || await _currentBranch({ fs: fs3, gitdir, test: true });
      const config = await GitConfigManager.get({ fs: fs3, gitdir });
      const remote = _remote || ref && await config.get(`branch.${ref}.remote`) || "origin";
      const url = _url || await config.get(`remote.${remote}.url`);
      if (typeof url === "undefined") {
        throw new MissingParameterError("remote OR url");
      }
      const remoteRef = _remoteRef || ref && await config.get(`branch.${ref}.merge`) || _ref || "HEAD";
      if (corsProxy === void 0) {
        corsProxy = await config.get("http.corsProxy");
      }
      const GitRemoteHTTP2 = GitRemoteManager.getRemoteHelperFor({ url });
      const remoteHTTP = await GitRemoteHTTP2.discover({
        http,
        onAuth,
        onAuthSuccess,
        onAuthFailure,
        corsProxy,
        service: "git-upload-pack",
        url,
        headers,
        protocolVersion: 1
      });
      const auth = remoteHTTP.auth;
      const remoteRefs = remoteHTTP.refs;
      if (remoteRefs.size === 0) {
        return {
          defaultBranch: null,
          fetchHead: null,
          fetchHeadDescription: null
        };
      }
      if (depth !== null && !remoteHTTP.capabilities.has("shallow")) {
        throw new RemoteCapabilityError("shallow", "depth");
      }
      if (since !== null && !remoteHTTP.capabilities.has("deepen-since")) {
        throw new RemoteCapabilityError("deepen-since", "since");
      }
      if (exclude.length > 0 && !remoteHTTP.capabilities.has("deepen-not")) {
        throw new RemoteCapabilityError("deepen-not", "exclude");
      }
      if (relative === true && !remoteHTTP.capabilities.has("deepen-relative")) {
        throw new RemoteCapabilityError("deepen-relative", "relative");
      }
      const { oid, fullref } = GitRefManager.resolveAgainstMap({
        ref: remoteRef,
        map: remoteRefs
      });
      for (const remoteRef2 of remoteRefs.keys()) {
        if (remoteRef2 === fullref || remoteRef2 === "HEAD" || remoteRef2.startsWith("refs/heads/") || tags && remoteRef2.startsWith("refs/tags/")) {
          continue;
        }
        remoteRefs.delete(remoteRef2);
      }
      const capabilities = filterCapabilities(
        [...remoteHTTP.capabilities],
        [
          "multi_ack_detailed",
          "no-done",
          "side-band-64k",
          // Note: I removed 'thin-pack' option since our code doesn't "fatten" packfiles,
          // which is necessary for compatibility with git. It was the cause of mysterious
          // 'fatal: pack has [x] unresolved deltas' errors that plagued us for some time.
          // isomorphic-git is perfectly happy with thin packfiles in .git/objects/pack but
          // canonical git it turns out is NOT.
          "ofs-delta",
          `agent=${pkg.agent}`
        ]
      );
      if (relative) capabilities.push("deepen-relative");
      const wants = singleBranch ? [oid] : remoteRefs.values();
      const haveRefs = singleBranch ? [ref] : await GitRefManager.listRefs({
        fs: fs3,
        gitdir,
        filepath: `refs`
      });
      let haves = [];
      for (let ref2 of haveRefs) {
        try {
          ref2 = await GitRefManager.expand({ fs: fs3, gitdir, ref: ref2 });
          const oid2 = await GitRefManager.resolve({ fs: fs3, gitdir, ref: ref2 });
          if (await hasObject({ fs: fs3, cache, gitdir, oid: oid2 })) {
            haves.push(oid2);
          }
        } catch (err) {
        }
      }
      haves = [...new Set(haves)];
      const oids = await GitShallowManager.read({ fs: fs3, gitdir });
      const shallows = remoteHTTP.capabilities.has("shallow") ? [...oids] : [];
      const packstream = writeUploadPackRequest({
        capabilities,
        wants,
        haves,
        shallows,
        depth,
        since,
        exclude
      });
      const packbuffer = Buffer.from(await collect(packstream));
      const raw = await GitRemoteHTTP2.connect({
        http,
        onProgress,
        corsProxy,
        service: "git-upload-pack",
        url,
        auth,
        body: [packbuffer],
        headers
      });
      const response = await parseUploadPackResponse(raw.body);
      if (raw.headers) {
        response.headers = raw.headers;
      }
      for (const oid2 of response.shallows) {
        if (!oids.has(oid2)) {
          try {
            const { object } = await _readObject({ fs: fs3, cache, gitdir, oid: oid2 });
            const commit2 = new GitCommit(object);
            const hasParents = await Promise.all(
              commit2.headers().parent.map((oid3) => hasObject({ fs: fs3, cache, gitdir, oid: oid3 }))
            );
            const haveAllParents = hasParents.length === 0 || hasParents.every((has) => has);
            if (!haveAllParents) {
              oids.add(oid2);
            }
          } catch (err) {
            oids.add(oid2);
          }
        }
      }
      for (const oid2 of response.unshallows) {
        oids.delete(oid2);
      }
      await GitShallowManager.write({ fs: fs3, gitdir, oids });
      if (singleBranch) {
        const refs = /* @__PURE__ */ new Map([[fullref, oid]]);
        const symrefs = /* @__PURE__ */ new Map();
        let bail = 10;
        let key = fullref;
        while (bail--) {
          const value = remoteHTTP.symrefs.get(key);
          if (value === void 0) break;
          symrefs.set(key, value);
          key = value;
        }
        const realRef = remoteRefs.get(key);
        if (realRef) {
          refs.set(key, realRef);
        }
        const { pruned } = await GitRefManager.updateRemoteRefs({
          fs: fs3,
          gitdir,
          remote,
          refs,
          symrefs,
          tags,
          prune
        });
        if (prune) {
          response.pruned = pruned;
        }
      } else {
        const { pruned } = await GitRefManager.updateRemoteRefs({
          fs: fs3,
          gitdir,
          remote,
          refs: remoteRefs,
          symrefs: remoteHTTP.symrefs,
          tags,
          prune,
          pruneTags
        });
        if (prune) {
          response.pruned = pruned;
        }
      }
      response.HEAD = remoteHTTP.symrefs.get("HEAD");
      if (response.HEAD === void 0) {
        const { oid: oid2 } = GitRefManager.resolveAgainstMap({
          ref: "HEAD",
          map: remoteRefs
        });
        for (const [key, value] of remoteRefs.entries()) {
          if (key !== "HEAD" && value === oid2) {
            response.HEAD = key;
            break;
          }
        }
      }
      const noun = fullref.startsWith("refs/tags") ? "tag" : "branch";
      response.FETCH_HEAD = {
        oid,
        description: `${noun} '${abbreviateRef(fullref)}' of ${url}`
      };
      if (onProgress || onMessage) {
        const lines = splitLines(response.progress);
        forAwait(lines, async (line) => {
          if (onMessage) await onMessage(line);
          if (onProgress) {
            const matches = line.match(/([^:]*).*\((\d+?)\/(\d+?)\)/);
            if (matches) {
              await onProgress({
                phase: matches[1].trim(),
                loaded: parseInt(matches[2], 10),
                total: parseInt(matches[3], 10)
              });
            }
          }
        });
      }
      const packfile = Buffer.from(await collect(response.packfile));
      if (raw.body.error) throw raw.body.error;
      const packfileSha = packfile.slice(-20).toString("hex");
      const res = {
        defaultBranch: response.HEAD,
        fetchHead: response.FETCH_HEAD.oid,
        fetchHeadDescription: response.FETCH_HEAD.description
      };
      if (response.headers) {
        res.headers = response.headers;
      }
      if (prune) {
        res.pruned = response.pruned;
      }
      if (packfileSha !== "" && !emptyPackfile(packfile)) {
        res.packfile = `objects/pack/pack-${packfileSha}.pack`;
        const fullpath = join2(gitdir, res.packfile);
        await fs3.write(fullpath, packfile);
        const getExternalRefDelta = (oid2) => _readObject({ fs: fs3, cache, gitdir, oid: oid2 });
        const idx = await GitPackIndex.fromPack({
          pack: packfile,
          getExternalRefDelta,
          onProgress
        });
        await fs3.write(fullpath.replace(/\.pack$/, ".idx"), await idx.toBuffer());
      }
      return res;
    }
    async function _init({
      fs: fs3,
      bare = false,
      dir,
      gitdir = bare ? dir : join2(dir, ".git"),
      defaultBranch = "master"
    }) {
      if (await fs3.exists(gitdir + "/config")) return;
      let folders = [
        "hooks",
        "info",
        "objects/info",
        "objects/pack",
        "refs/heads",
        "refs/tags"
      ];
      folders = folders.map((dir2) => gitdir + "/" + dir2);
      for (const folder of folders) {
        await fs3.mkdir(folder);
      }
      await fs3.write(
        gitdir + "/config",
        `[core]
	repositoryformatversion = 0
	filemode = false
	bare = ${bare}
` + (bare ? "" : "	logallrefupdates = true\n") + "	symlinks = false\n	ignorecase = true\n"
      );
      await fs3.write(gitdir + "/HEAD", `ref: refs/heads/${defaultBranch}
`);
    }
    async function _clone({
      fs: fs3,
      cache,
      http,
      onProgress,
      onMessage,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      onPostCheckout,
      dir,
      gitdir,
      url,
      corsProxy,
      ref,
      remote,
      depth,
      since,
      exclude,
      relative,
      singleBranch,
      noCheckout,
      noTags,
      headers,
      nonBlocking,
      batchSize = 100
    }) {
      try {
        await _init({ fs: fs3, gitdir });
        await _addRemote({ fs: fs3, gitdir, remote, url, force: false });
        if (corsProxy) {
          const config = await GitConfigManager.get({ fs: fs3, gitdir });
          await config.set(`http.corsProxy`, corsProxy);
          await GitConfigManager.save({ fs: fs3, gitdir, config });
        }
        const { defaultBranch, fetchHead } = await _fetch({
          fs: fs3,
          cache,
          http,
          onProgress,
          onMessage,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          gitdir,
          ref,
          remote,
          corsProxy,
          depth,
          since,
          exclude,
          relative,
          singleBranch,
          headers,
          tags: !noTags
        });
        if (fetchHead === null) return;
        ref = ref || defaultBranch;
        ref = ref.replace("refs/heads/", "");
        await _checkout({
          fs: fs3,
          cache,
          onProgress,
          onPostCheckout,
          dir,
          gitdir,
          ref,
          remote,
          noCheckout,
          nonBlocking,
          batchSize
        });
      } catch (err) {
        await fs3.rmdir(gitdir, { recursive: true, maxRetries: 10 }).catch(() => void 0);
        throw err;
      }
    }
    async function clone({
      fs: fs3,
      http,
      onProgress,
      onMessage,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      onPostCheckout,
      dir,
      gitdir = join2(dir, ".git"),
      url,
      corsProxy = void 0,
      ref = void 0,
      remote = "origin",
      depth = void 0,
      since = void 0,
      exclude = [],
      relative = false,
      singleBranch = false,
      noCheckout = false,
      noTags = false,
      headers = {},
      cache = {},
      nonBlocking = false,
      batchSize = 100
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("http", http);
        assertParameter("gitdir", gitdir);
        if (!noCheckout) {
          assertParameter("dir", dir);
        }
        assertParameter("url", url);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _clone({
          fs: fsp,
          cache,
          http,
          onProgress,
          onMessage,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          onPostCheckout,
          dir,
          gitdir: updatedGitdir,
          url,
          corsProxy,
          ref,
          remote,
          depth,
          since,
          exclude,
          relative,
          singleBranch,
          noCheckout,
          noTags,
          headers,
          nonBlocking,
          batchSize
        });
      } catch (err) {
        err.caller = "git.clone";
        throw err;
      }
    }
    async function commit({
      fs: _fs,
      onSign,
      dir,
      gitdir = join2(dir, ".git"),
      message,
      author,
      committer,
      signingKey,
      amend = false,
      dryRun = false,
      noUpdateBranch = false,
      ref,
      parent,
      tree,
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        if (!amend) {
          assertParameter("message", message);
        }
        if (signingKey) {
          assertParameter("onSign", onSign);
        }
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        return await _commit({
          fs: fs3,
          cache,
          onSign,
          gitdir: updatedGitdir,
          message,
          author,
          committer,
          signingKey,
          amend,
          dryRun,
          noUpdateBranch,
          ref,
          parent,
          tree
        });
      } catch (err) {
        err.caller = "git.commit";
        throw err;
      }
    }
    async function currentBranch({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      fullname = false,
      test = false
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _currentBranch({
          fs: fsp,
          gitdir: updatedGitdir,
          fullname,
          test
        });
      } catch (err) {
        err.caller = "git.currentBranch";
        throw err;
      }
    }
    async function _deleteBranch({ fs: fs3, gitdir, ref }) {
      ref = ref.startsWith("refs/heads/") ? ref : `refs/heads/${ref}`;
      const exist = await GitRefManager.exists({ fs: fs3, gitdir, ref });
      if (!exist) {
        throw new NotFoundError(ref);
      }
      const fullRef = await GitRefManager.expand({ fs: fs3, gitdir, ref });
      const currentRef = await _currentBranch({ fs: fs3, gitdir, fullname: true });
      if (fullRef === currentRef) {
        const value = await GitRefManager.resolve({ fs: fs3, gitdir, ref: fullRef });
        await GitRefManager.writeRef({ fs: fs3, gitdir, ref: "HEAD", value });
      }
      await GitRefManager.deleteRef({ fs: fs3, gitdir, ref: fullRef });
      const abbrevRef = abbreviateRef(ref);
      const config = await GitConfigManager.get({ fs: fs3, gitdir });
      await config.deleteSection("branch", abbrevRef);
      await GitConfigManager.save({ fs: fs3, gitdir, config });
    }
    async function deleteBranch2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      ref
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("ref", ref);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _deleteBranch({
          fs: fsp,
          gitdir: updatedGitdir,
          ref
        });
      } catch (err) {
        err.caller = "git.deleteBranch";
        throw err;
      }
    }
    async function deleteRef({ fs: fs3, dir, gitdir = join2(dir, ".git"), ref }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("ref", ref);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        await GitRefManager.deleteRef({ fs: fsp, gitdir: updatedGitdir, ref });
      } catch (err) {
        err.caller = "git.deleteRef";
        throw err;
      }
    }
    async function _deleteRemote({ fs: fs3, gitdir, remote }) {
      const config = await GitConfigManager.get({ fs: fs3, gitdir });
      await config.deleteSection("remote", remote);
      await GitConfigManager.save({ fs: fs3, gitdir, config });
    }
    async function deleteRemote({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      remote
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("remote", remote);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _deleteRemote({
          fs: fsp,
          gitdir: updatedGitdir,
          remote
        });
      } catch (err) {
        err.caller = "git.deleteRemote";
        throw err;
      }
    }
    async function _deleteTag({ fs: fs3, gitdir, ref }) {
      ref = ref.startsWith("refs/tags/") ? ref : `refs/tags/${ref}`;
      await GitRefManager.deleteRef({ fs: fs3, gitdir, ref });
    }
    async function deleteTag({ fs: fs3, dir, gitdir = join2(dir, ".git"), ref }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("ref", ref);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _deleteTag({
          fs: fsp,
          gitdir: updatedGitdir,
          ref
        });
      } catch (err) {
        err.caller = "git.deleteTag";
        throw err;
      }
    }
    async function expandOidLoose({ fs: fs3, gitdir, oid: short }) {
      const prefix = short.slice(0, 2);
      const objectsSuffixes = await fs3.readdir(`${gitdir}/objects/${prefix}`);
      return objectsSuffixes.map((suffix) => `${prefix}${suffix}`).filter((_oid) => _oid.startsWith(short));
    }
    async function expandOidPacked({
      fs: fs3,
      cache,
      gitdir,
      oid: short,
      getExternalRefDelta
    }) {
      const results = [];
      let list = await fs3.readdir(join2(gitdir, "objects/pack"));
      list = list.filter((x) => x.endsWith(".idx"));
      for (const filename of list) {
        const indexFile = `${gitdir}/objects/pack/${filename}`;
        const p = await readPackIndex({
          fs: fs3,
          cache,
          filename: indexFile,
          getExternalRefDelta
        });
        if (p.error) throw new InternalError(p.error);
        for (const oid of p.offsets.keys()) {
          if (oid.startsWith(short)) results.push(oid);
        }
      }
      return results;
    }
    async function _expandOid({ fs: fs3, cache, gitdir, oid: short }) {
      const getExternalRefDelta = (oid) => _readObject({ fs: fs3, cache, gitdir, oid });
      const results = await expandOidLoose({ fs: fs3, gitdir, oid: short });
      const packedOids = await expandOidPacked({
        fs: fs3,
        cache,
        gitdir,
        oid: short,
        getExternalRefDelta
      });
      for (const packedOid of packedOids) {
        if (results.indexOf(packedOid) === -1) {
          results.push(packedOid);
        }
      }
      if (results.length === 1) {
        return results[0];
      }
      if (results.length > 1) {
        throw new AmbiguousError("oids", short, results);
      }
      throw new NotFoundError(`an object matching "${short}"`);
    }
    async function expandOid2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      oid,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _expandOid({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          oid
        });
      } catch (err) {
        err.caller = "git.expandOid";
        throw err;
      }
    }
    async function expandRef({ fs: fs3, dir, gitdir = join2(dir, ".git"), ref }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await GitRefManager.expand({
          fs: fsp,
          gitdir: updatedGitdir,
          ref
        });
      } catch (err) {
        err.caller = "git.expandRef";
        throw err;
      }
    }
    async function _findMergeBase({ fs: fs3, cache, gitdir, oids }) {
      const visits = {};
      const passes = oids.length;
      let heads = oids.map((oid, index2) => ({ index: index2, oid }));
      while (heads.length) {
        const result = /* @__PURE__ */ new Set();
        for (const { oid, index: index2 } of heads) {
          if (!visits[oid]) visits[oid] = /* @__PURE__ */ new Set();
          visits[oid].add(index2);
          if (visits[oid].size === passes) {
            result.add(oid);
          }
        }
        if (result.size > 0) {
          return [...result];
        }
        const newheads = /* @__PURE__ */ new Map();
        for (const { oid, index: index2 } of heads) {
          try {
            const { object } = await _readObject({ fs: fs3, cache, gitdir, oid });
            const commit2 = GitCommit.from(object);
            const { parent } = commit2.parseHeaders();
            for (const oid2 of parent) {
              if (!visits[oid2] || !visits[oid2].has(index2)) {
                newheads.set(oid2 + ":" + index2, { oid: oid2, index: index2 });
              }
            }
          } catch (err) {
          }
        }
        heads = Array.from(newheads.values());
      }
      return [];
    }
    async function _merge({
      fs: fs3,
      cache,
      dir,
      gitdir,
      ours,
      theirs,
      fastForward: fastForward2 = true,
      fastForwardOnly = false,
      dryRun = false,
      noUpdateBranch = false,
      abortOnConflict = true,
      message,
      author,
      committer,
      signingKey,
      onSign,
      mergeDriver,
      allowUnrelatedHistories = false
    }) {
      if (ours === void 0) {
        ours = await _currentBranch({ fs: fs3, gitdir, fullname: true });
      }
      ours = await GitRefManager.expand({
        fs: fs3,
        gitdir,
        ref: ours
      });
      theirs = await GitRefManager.expand({
        fs: fs3,
        gitdir,
        ref: theirs
      });
      const ourOid = await GitRefManager.resolve({
        fs: fs3,
        gitdir,
        ref: ours
      });
      const theirOid = await GitRefManager.resolve({
        fs: fs3,
        gitdir,
        ref: theirs
      });
      const baseOids = await _findMergeBase({
        fs: fs3,
        cache,
        gitdir,
        oids: [ourOid, theirOid]
      });
      if (baseOids.length !== 1) {
        if (baseOids.length === 0 && allowUnrelatedHistories) {
          baseOids.push("4b825dc642cb6eb9a060e54bf8d69288fbee4904");
        } else {
          throw new MergeNotSupportedError();
        }
      }
      const baseOid = baseOids[0];
      if (baseOid === theirOid) {
        return {
          oid: ourOid,
          alreadyMerged: true
        };
      }
      if (fastForward2 && baseOid === ourOid) {
        if (!dryRun && !noUpdateBranch) {
          await GitRefManager.writeRef({ fs: fs3, gitdir, ref: ours, value: theirOid });
        }
        return {
          oid: theirOid,
          fastForward: true
        };
      } else {
        if (fastForwardOnly) {
          throw new FastForwardError();
        }
        const tree = await GitIndexManager.acquire(
          { fs: fs3, gitdir, cache, allowUnmerged: false },
          async (index2) => {
            return mergeTree({
              fs: fs3,
              cache,
              dir,
              gitdir,
              index: index2,
              ourOid,
              theirOid,
              baseOid,
              ourName: abbreviateRef(ours),
              baseName: "base",
              theirName: abbreviateRef(theirs),
              dryRun,
              abortOnConflict,
              mergeDriver
            });
          }
        );
        if (tree instanceof MergeConflictError) throw tree;
        if (!message) {
          message = `Merge branch '${abbreviateRef(theirs)}' into ${abbreviateRef(
            ours
          )}`;
        }
        const oid = await _commit({
          fs: fs3,
          cache,
          gitdir,
          message,
          ref: ours,
          tree,
          parent: [ourOid, theirOid],
          author,
          committer,
          signingKey,
          onSign,
          dryRun,
          noUpdateBranch
        });
        return {
          oid,
          tree,
          mergeCommit: true
        };
      }
    }
    async function _pull({
      fs: fs3,
      cache,
      http,
      onProgress,
      onMessage,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      dir,
      gitdir,
      ref,
      url,
      remote,
      remoteRef,
      prune,
      pruneTags,
      fastForward: fastForward2,
      fastForwardOnly,
      corsProxy,
      singleBranch,
      headers,
      author,
      committer,
      signingKey
    }) {
      try {
        if (!ref) {
          const head = await _currentBranch({ fs: fs3, gitdir });
          if (!head) {
            throw new MissingParameterError("ref");
          }
          ref = head;
        }
        const { fetchHead, fetchHeadDescription } = await _fetch({
          fs: fs3,
          cache,
          http,
          onProgress,
          onMessage,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          gitdir,
          corsProxy,
          ref,
          url,
          remote,
          remoteRef,
          singleBranch,
          headers,
          prune,
          pruneTags
        });
        await _merge({
          fs: fs3,
          cache,
          gitdir,
          ours: ref,
          theirs: fetchHead,
          fastForward: fastForward2,
          fastForwardOnly,
          message: `Merge ${fetchHeadDescription}`,
          author,
          committer,
          signingKey,
          dryRun: false,
          noUpdateBranch: false
        });
        await _checkout({
          fs: fs3,
          cache,
          onProgress,
          dir,
          gitdir,
          ref,
          remote,
          noCheckout: false
        });
      } catch (err) {
        err.caller = "git.pull";
        throw err;
      }
    }
    async function fastForward({
      fs: fs3,
      http,
      onProgress,
      onMessage,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      url,
      remote,
      remoteRef,
      corsProxy,
      singleBranch,
      headers = {},
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("http", http);
        assertParameter("gitdir", gitdir);
        const thisWillNotBeUsed = {
          name: "",
          email: "",
          timestamp: Date.now(),
          timezoneOffset: 0
        };
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _pull({
          fs: fsp,
          cache,
          http,
          onProgress,
          onMessage,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          dir,
          gitdir: updatedGitdir,
          ref,
          url,
          remote,
          remoteRef,
          fastForwardOnly: true,
          corsProxy,
          singleBranch,
          headers,
          author: thisWillNotBeUsed,
          committer: thisWillNotBeUsed
        });
      } catch (err) {
        err.caller = "git.fastForward";
        throw err;
      }
    }
    async function fetch({
      fs: fs3,
      http,
      onProgress,
      onMessage,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      remote,
      remoteRef,
      url,
      corsProxy,
      depth = null,
      since = null,
      exclude = [],
      relative = false,
      tags = false,
      singleBranch = false,
      headers = {},
      prune = false,
      pruneTags = false,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("http", http);
        assertParameter("gitdir", gitdir);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _fetch({
          fs: fsp,
          cache,
          http,
          onProgress,
          onMessage,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          gitdir: updatedGitdir,
          ref,
          remote,
          remoteRef,
          url,
          corsProxy,
          depth,
          since,
          exclude,
          relative,
          tags,
          singleBranch,
          headers,
          prune,
          pruneTags
        });
      } catch (err) {
        err.caller = "git.fetch";
        throw err;
      }
    }
    async function findMergeBase2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      oids,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("oids", oids);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _findMergeBase({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          oids
        });
      } catch (err) {
        err.caller = "git.findMergeBase";
        throw err;
      }
    }
    async function _findRoot({ fs: fs3, filepath }) {
      if (await fs3.exists(join2(filepath, ".git"))) {
        return filepath;
      } else {
        const parent = dirname(filepath);
        if (parent === filepath) {
          throw new NotFoundError(`git root for ${filepath}`);
        }
        return _findRoot({ fs: fs3, filepath: parent });
      }
    }
    async function findRoot({ fs: fs3, filepath }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("filepath", filepath);
        return await _findRoot({ fs: new FileSystem(fs3), filepath });
      } catch (err) {
        err.caller = "git.findRoot";
        throw err;
      }
    }
    async function getConfig({ fs: fs3, dir, gitdir = join2(dir, ".git"), path: path3 }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("path", path3);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _getConfig({
          fs: fsp,
          gitdir: updatedGitdir,
          path: path3
        });
      } catch (err) {
        err.caller = "git.getConfig";
        throw err;
      }
    }
    async function _getConfigAll({ fs: fs3, gitdir, path: path3 }) {
      const config = await GitConfigManager.get({ fs: fs3, gitdir });
      return config.getall(path3);
    }
    async function getConfigAll({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      path: path3
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("path", path3);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _getConfigAll({
          fs: fsp,
          gitdir: updatedGitdir,
          path: path3
        });
      } catch (err) {
        err.caller = "git.getConfigAll";
        throw err;
      }
    }
    async function getRemoteInfo({
      http,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      corsProxy,
      url,
      headers = {},
      forPush = false
    }) {
      try {
        assertParameter("http", http);
        assertParameter("url", url);
        const GitRemoteHTTP2 = GitRemoteManager.getRemoteHelperFor({ url });
        const remote = await GitRemoteHTTP2.discover({
          http,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          corsProxy,
          service: forPush ? "git-receive-pack" : "git-upload-pack",
          url,
          headers,
          protocolVersion: 1
        });
        const result = {
          capabilities: [...remote.capabilities]
        };
        for (const [ref, oid] of remote.refs) {
          const parts = ref.split("/");
          const last = parts.pop();
          let o = result;
          for (const part of parts) {
            o[part] = o[part] || {};
            o = o[part];
          }
          o[last] = oid;
        }
        for (const [symref, ref] of remote.symrefs) {
          const parts = symref.split("/");
          const last = parts.pop();
          let o = result;
          for (const part of parts) {
            o[part] = o[part] || {};
            o = o[part];
          }
          o[last] = ref;
        }
        return result;
      } catch (err) {
        err.caller = "git.getRemoteInfo";
        throw err;
      }
    }
    function formatInfoRefs(remote, prefix, symrefs, peelTags) {
      const refs = [];
      for (const [key, value] of remote.refs) {
        if (prefix && !key.startsWith(prefix)) continue;
        if (key.endsWith("^{}")) {
          if (peelTags) {
            const _key = key.replace("^{}", "");
            const last = refs[refs.length - 1];
            const r = last.ref === _key ? last : refs.find((x) => x.ref === _key);
            if (r === void 0) {
              throw new Error("I did not expect this to happen");
            }
            r.peeled = value;
          }
          continue;
        }
        const ref = { ref: key, oid: value };
        if (symrefs) {
          if (remote.symrefs.has(key)) {
            ref.target = remote.symrefs.get(key);
          }
        }
        refs.push(ref);
      }
      return refs;
    }
    async function getRemoteInfo2({
      http,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      corsProxy,
      url,
      headers = {},
      forPush = false,
      protocolVersion = 2
    }) {
      try {
        assertParameter("http", http);
        assertParameter("url", url);
        const GitRemoteHTTP2 = GitRemoteManager.getRemoteHelperFor({ url });
        const remote = await GitRemoteHTTP2.discover({
          http,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          corsProxy,
          service: forPush ? "git-receive-pack" : "git-upload-pack",
          url,
          headers,
          protocolVersion
        });
        if (remote.protocolVersion === 2) {
          return {
            protocolVersion: remote.protocolVersion,
            capabilities: remote.capabilities2
          };
        }
        const capabilities = {};
        for (const cap of remote.capabilities) {
          const [key, value] = cap.split("=");
          if (value) {
            capabilities[key] = value;
          } else {
            capabilities[key] = true;
          }
        }
        return {
          protocolVersion: 1,
          capabilities,
          refs: formatInfoRefs(remote, void 0, true, true)
        };
      } catch (err) {
        err.caller = "git.getRemoteInfo2";
        throw err;
      }
    }
    async function hashObject({
      type,
      object,
      format = "content",
      oid = void 0
    }) {
      if (format !== "deflated") {
        if (format !== "wrapped") {
          object = GitObject.wrap({ type, object });
        }
        oid = await shasum(object);
      }
      return { oid, object };
    }
    async function hashBlob({ object }) {
      try {
        assertParameter("object", object);
        if (typeof object === "string") {
          object = Buffer.from(object, "utf8");
        } else if (!(object instanceof Uint8Array)) {
          object = new Uint8Array(object);
        }
        const type = "blob";
        const { oid, object: _object } = await hashObject({
          type,
          format: "content",
          object
        });
        return { oid, type, object: _object, format: "wrapped" };
      } catch (err) {
        err.caller = "git.hashBlob";
        throw err;
      }
    }
    async function _indexPack({
      fs: fs3,
      cache,
      onProgress,
      dir,
      gitdir,
      filepath
    }) {
      try {
        filepath = join2(dir, filepath);
        const pack = await fs3.read(filepath);
        const getExternalRefDelta = (oid) => _readObject({ fs: fs3, cache, gitdir, oid });
        const idx = await GitPackIndex.fromPack({
          pack,
          getExternalRefDelta,
          onProgress
        });
        await fs3.write(filepath.replace(/\.pack$/, ".idx"), await idx.toBuffer());
        return {
          oids: [...idx.hashes]
        };
      } catch (err) {
        err.caller = "git.indexPack";
        throw err;
      }
    }
    async function indexPack({
      fs: fs3,
      onProgress,
      dir,
      gitdir = join2(dir, ".git"),
      filepath,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("dir", dir);
        assertParameter("gitdir", dir);
        assertParameter("filepath", filepath);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _indexPack({
          fs: fsp,
          cache,
          onProgress,
          dir,
          gitdir: updatedGitdir,
          filepath
        });
      } catch (err) {
        err.caller = "git.indexPack";
        throw err;
      }
    }
    async function init2({
      fs: fs3,
      bare = false,
      dir,
      gitdir = bare ? dir : join2(dir, ".git"),
      defaultBranch = "master"
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        if (!bare) {
          assertParameter("dir", dir);
        }
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _init({
          fs: fsp,
          bare,
          dir,
          gitdir: updatedGitdir,
          defaultBranch
        });
      } catch (err) {
        err.caller = "git.init";
        throw err;
      }
    }
    async function _isDescendent({
      fs: fs3,
      cache,
      gitdir,
      oid,
      ancestor,
      depth
    }) {
      const shallows = await GitShallowManager.read({ fs: fs3, gitdir });
      if (!oid) {
        throw new MissingParameterError("oid");
      }
      if (!ancestor) {
        throw new MissingParameterError("ancestor");
      }
      if (oid === ancestor) return false;
      const queue = [oid];
      const visited = /* @__PURE__ */ new Set();
      let searchdepth = 0;
      while (queue.length) {
        if (searchdepth++ === depth) {
          throw new MaxDepthError(depth);
        }
        const oid2 = queue.shift();
        const { type, object } = await _readObject({
          fs: fs3,
          cache,
          gitdir,
          oid: oid2
        });
        if (type !== "commit") {
          throw new ObjectTypeError(oid2, type, "commit");
        }
        const commit2 = GitCommit.from(object).parse();
        for (const parent of commit2.parent) {
          if (parent === ancestor) return true;
        }
        if (!shallows.has(oid2)) {
          for (const parent of commit2.parent) {
            if (!visited.has(parent)) {
              queue.push(parent);
              visited.add(parent);
            }
          }
        }
      }
      return false;
    }
    async function isDescendent2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      oid,
      ancestor,
      depth = -1,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        assertParameter("ancestor", ancestor);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _isDescendent({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          oid,
          ancestor,
          depth
        });
      } catch (err) {
        err.caller = "git.isDescendent";
        throw err;
      }
    }
    async function isIgnored({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      filepath
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("dir", dir);
        assertParameter("gitdir", gitdir);
        assertParameter("filepath", filepath);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return GitIgnoreManager.isIgnored({
          fs: fsp,
          dir,
          gitdir: updatedGitdir,
          filepath
        });
      } catch (err) {
        err.caller = "git.isIgnored";
        throw err;
      }
    }
    async function listBranches2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      remote
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return GitRefManager.listBranches({
          fs: fsp,
          gitdir: updatedGitdir,
          remote
        });
      } catch (err) {
        err.caller = "git.listBranches";
        throw err;
      }
    }
    async function _listFiles({ fs: fs3, gitdir, ref, cache }) {
      if (ref) {
        const oid = await GitRefManager.resolve({ gitdir, fs: fs3, ref });
        const filenames = [];
        await accumulateFilesFromOid({
          fs: fs3,
          cache,
          gitdir,
          oid,
          filenames,
          prefix: ""
        });
        return filenames;
      } else {
        return GitIndexManager.acquire(
          { fs: fs3, gitdir, cache },
          async function(index2) {
            return index2.entries.map((x) => x.path);
          }
        );
      }
    }
    async function accumulateFilesFromOid({
      fs: fs3,
      cache,
      gitdir,
      oid,
      filenames,
      prefix
    }) {
      const { tree } = await _readTree({ fs: fs3, cache, gitdir, oid });
      for (const entry of tree) {
        if (entry.type === "tree") {
          await accumulateFilesFromOid({
            fs: fs3,
            cache,
            gitdir,
            oid: entry.oid,
            filenames,
            prefix: join2(prefix, entry.path)
          });
        } else {
          filenames.push(join2(prefix, entry.path));
        }
      }
    }
    async function listFiles({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _listFiles({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          ref
        });
      } catch (err) {
        err.caller = "git.listFiles";
        throw err;
      }
    }
    async function _listNotes({ fs: fs3, cache, gitdir, ref }) {
      let parent;
      try {
        parent = await GitRefManager.resolve({ gitdir, fs: fs3, ref });
      } catch (err) {
        if (err instanceof NotFoundError) {
          return [];
        }
      }
      const result = await _readTree({
        fs: fs3,
        cache,
        gitdir,
        oid: parent
      });
      const notes = result.tree.map((entry) => ({
        target: entry.path,
        note: entry.oid
      }));
      return notes;
    }
    async function listNotes({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      ref = "refs/notes/commits",
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _listNotes({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          ref
        });
      } catch (err) {
        err.caller = "git.listNotes";
        throw err;
      }
    }
    async function listRefs({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      filepath
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return GitRefManager.listRefs({ fs: fsp, gitdir: updatedGitdir, filepath });
      } catch (err) {
        err.caller = "git.listRefs";
        throw err;
      }
    }
    async function _listRemotes({ fs: fs3, gitdir }) {
      const config = await GitConfigManager.get({ fs: fs3, gitdir });
      const remoteNames = await config.getSubsections("remote");
      const remotes = Promise.all(
        remoteNames.map(async (remote) => {
          const url = await config.get(`remote.${remote}.url`);
          return { remote, url };
        })
      );
      return remotes;
    }
    async function listRemotes({ fs: fs3, dir, gitdir = join2(dir, ".git") }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _listRemotes({
          fs: fsp,
          gitdir: updatedGitdir
        });
      } catch (err) {
        err.caller = "git.listRemotes";
        throw err;
      }
    }
    async function parseListRefsResponse(stream) {
      const read = GitPktLine.streamReader(stream);
      const refs = [];
      let line;
      while (true) {
        line = await read();
        if (line === true) break;
        if (line === null) continue;
        line = line.toString("utf8").replace(/\n$/, "");
        const [oid, ref, ...attrs] = line.split(" ");
        const r = { ref, oid };
        for (const attr of attrs) {
          const [name, value] = attr.split(":");
          if (name === "symref-target") {
            r.target = value;
          } else if (name === "peeled") {
            r.peeled = value;
          }
        }
        refs.push(r);
      }
      return refs;
    }
    async function writeListRefsRequest({ prefix, symrefs, peelTags }) {
      const packstream = [];
      packstream.push(GitPktLine.encode("command=ls-refs\n"));
      packstream.push(GitPktLine.encode(`agent=${pkg.agent}
`));
      if (peelTags || symrefs || prefix) {
        packstream.push(GitPktLine.delim());
      }
      if (peelTags) packstream.push(GitPktLine.encode("peel"));
      if (symrefs) packstream.push(GitPktLine.encode("symrefs"));
      if (prefix) packstream.push(GitPktLine.encode(`ref-prefix ${prefix}`));
      packstream.push(GitPktLine.flush());
      return packstream;
    }
    async function listServerRefs({
      http,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      corsProxy,
      url,
      headers = {},
      forPush = false,
      protocolVersion = 2,
      prefix,
      symrefs,
      peelTags
    }) {
      try {
        assertParameter("http", http);
        assertParameter("url", url);
        const remote = await GitRemoteHTTP.discover({
          http,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          corsProxy,
          service: forPush ? "git-receive-pack" : "git-upload-pack",
          url,
          headers,
          protocolVersion
        });
        if (remote.protocolVersion === 1) {
          return formatInfoRefs(remote, prefix, symrefs, peelTags);
        }
        const body = await writeListRefsRequest({ prefix, symrefs, peelTags });
        const res = await GitRemoteHTTP.connect({
          http,
          auth: remote.auth,
          headers,
          corsProxy,
          service: forPush ? "git-receive-pack" : "git-upload-pack",
          url,
          body
        });
        return parseListRefsResponse(res.body);
      } catch (err) {
        err.caller = "git.listServerRefs";
        throw err;
      }
    }
    async function listTags({ fs: fs3, dir, gitdir = join2(dir, ".git") }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return GitRefManager.listTags({ fs: fsp, gitdir: updatedGitdir });
      } catch (err) {
        err.caller = "git.listTags";
        throw err;
      }
    }
    function compareAge(a, b) {
      return a.committer.timestamp - b.committer.timestamp;
    }
    var EMPTY_OID = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
    async function resolveFileIdInTree({ fs: fs3, cache, gitdir, oid, fileId }) {
      if (fileId === EMPTY_OID) return;
      const _oid = oid;
      let filepath;
      const result = await resolveTree({ fs: fs3, cache, gitdir, oid });
      const tree = result.tree;
      if (fileId === result.oid) {
        filepath = result.path;
      } else {
        filepath = await _resolveFileId({
          fs: fs3,
          cache,
          gitdir,
          tree,
          fileId,
          oid: _oid
        });
        if (Array.isArray(filepath)) {
          if (filepath.length === 0) filepath = void 0;
          else if (filepath.length === 1) filepath = filepath[0];
        }
      }
      return filepath;
    }
    async function _resolveFileId({
      fs: fs3,
      cache,
      gitdir,
      tree,
      fileId,
      oid,
      filepaths = [],
      parentPath = ""
    }) {
      const walks = tree.entries().map(function(entry) {
        let result;
        if (entry.oid === fileId) {
          result = join2(parentPath, entry.path);
          filepaths.push(result);
        } else if (entry.type === "tree") {
          result = _readObject({
            fs: fs3,
            cache,
            gitdir,
            oid: entry.oid
          }).then(function({ object }) {
            return _resolveFileId({
              fs: fs3,
              cache,
              gitdir,
              tree: GitTree.from(object),
              fileId,
              oid,
              filepaths,
              parentPath: join2(parentPath, entry.path)
            });
          });
        }
        return result;
      });
      await Promise.all(walks);
      return filepaths;
    }
    async function _log({
      fs: fs3,
      cache,
      gitdir,
      filepath,
      ref,
      depth,
      since,
      force,
      follow
    }) {
      const sinceTimestamp = typeof since === "undefined" ? void 0 : Math.floor(since.valueOf() / 1e3);
      const commits = [];
      const shallowCommits = await GitShallowManager.read({ fs: fs3, gitdir });
      const oid = await GitRefManager.resolve({ fs: fs3, gitdir, ref });
      const tips = [await _readCommit({ fs: fs3, cache, gitdir, oid })];
      let lastFileOid;
      let lastCommit;
      let isOk;
      function endCommit(commit2) {
        if (isOk && filepath) commits.push(commit2);
      }
      while (tips.length > 0) {
        const commit2 = tips.pop();
        if (sinceTimestamp !== void 0 && commit2.commit.committer.timestamp <= sinceTimestamp) {
          break;
        }
        if (filepath) {
          let vFileOid;
          try {
            vFileOid = await resolveFilepath({
              fs: fs3,
              cache,
              gitdir,
              oid: commit2.commit.tree,
              filepath
            });
            if (lastCommit && lastFileOid !== vFileOid) {
              commits.push(lastCommit);
            }
            lastFileOid = vFileOid;
            lastCommit = commit2;
            isOk = true;
          } catch (e) {
            if (e instanceof NotFoundError) {
              let found = follow && lastFileOid;
              if (found) {
                found = await resolveFileIdInTree({
                  fs: fs3,
                  cache,
                  gitdir,
                  oid: commit2.commit.tree,
                  fileId: lastFileOid
                });
                if (found) {
                  if (Array.isArray(found)) {
                    if (lastCommit) {
                      const lastFound = await resolveFileIdInTree({
                        fs: fs3,
                        cache,
                        gitdir,
                        oid: lastCommit.commit.tree,
                        fileId: lastFileOid
                      });
                      if (Array.isArray(lastFound)) {
                        found = found.filter((p) => lastFound.indexOf(p) === -1);
                        if (found.length === 1) {
                          found = found[0];
                          filepath = found;
                          if (lastCommit) commits.push(lastCommit);
                        } else {
                          found = false;
                          if (lastCommit) commits.push(lastCommit);
                          break;
                        }
                      }
                    }
                  } else {
                    filepath = found;
                    if (lastCommit) commits.push(lastCommit);
                  }
                }
              }
              if (!found) {
                if (isOk && lastFileOid) {
                  commits.push(lastCommit);
                  if (!force) break;
                }
                if (!force && !follow) throw e;
              }
              lastCommit = commit2;
              isOk = false;
            } else throw e;
          }
        } else {
          commits.push(commit2);
        }
        if (depth !== void 0 && commits.length === depth) {
          endCommit(commit2);
          break;
        }
        if (!shallowCommits.has(commit2.oid)) {
          for (const oid2 of commit2.commit.parent) {
            const commit3 = await _readCommit({ fs: fs3, cache, gitdir, oid: oid2 });
            if (!tips.map((commit4) => commit4.oid).includes(commit3.oid)) {
              tips.push(commit3);
            }
          }
        }
        if (tips.length === 0) {
          endCommit(commit2);
        }
        tips.sort((a, b) => compareAge(a.commit, b.commit));
      }
      return commits;
    }
    async function log2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      filepath,
      ref = "HEAD",
      depth,
      since,
      // Date
      force,
      follow,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _log({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          filepath,
          ref,
          depth,
          since,
          force,
          follow
        });
      } catch (err) {
        err.caller = "git.log";
        throw err;
      }
    }
    async function merge2({
      fs: _fs,
      onSign,
      dir,
      gitdir = join2(dir, ".git"),
      ours,
      theirs,
      fastForward: fastForward2 = true,
      fastForwardOnly = false,
      dryRun = false,
      noUpdateBranch = false,
      abortOnConflict = true,
      message,
      author: _author,
      committer: _committer,
      signingKey,
      cache = {},
      mergeDriver,
      allowUnrelatedHistories = false
    }) {
      try {
        assertParameter("fs", _fs);
        if (signingKey) {
          assertParameter("onSign", onSign);
        }
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        const author = await normalizeAuthorObject({
          fs: fs3,
          gitdir: updatedGitdir,
          author: _author
        });
        if (!author && (!fastForwardOnly || !fastForward2)) {
          throw new MissingNameError("author");
        }
        const committer = await normalizeCommitterObject({
          fs: fs3,
          gitdir: updatedGitdir,
          author,
          committer: _committer
        });
        if (!committer && (!fastForwardOnly || !fastForward2)) {
          throw new MissingNameError("committer");
        }
        return await _merge({
          fs: fs3,
          cache,
          dir,
          gitdir: updatedGitdir,
          ours,
          theirs,
          fastForward: fastForward2,
          fastForwardOnly,
          dryRun,
          noUpdateBranch,
          abortOnConflict,
          message,
          author,
          committer,
          signingKey,
          onSign,
          mergeDriver,
          allowUnrelatedHistories
        });
      } catch (err) {
        err.caller = "git.merge";
        throw err;
      }
    }
    var types = {
      commit: 16,
      tree: 32,
      blob: 48,
      tag: 64,
      ofs_delta: 96,
      ref_delta: 112
    };
    async function _pack({
      fs: fs3,
      cache,
      dir,
      gitdir = join2(dir, ".git"),
      oids
    }) {
      const hash = new Hash();
      const outputStream = [];
      function write(chunk, enc) {
        const buff = Buffer.from(chunk, enc);
        outputStream.push(buff);
        hash.update(buff);
      }
      async function writeObject2({ stype, object }) {
        const type = types[stype];
        let length = object.length;
        let multibyte = length > 15 ? 128 : 0;
        const lastFour = length & 15;
        length = length >>> 4;
        let byte = (multibyte | type | lastFour).toString(16);
        write(byte, "hex");
        while (multibyte) {
          multibyte = length > 127 ? 128 : 0;
          byte = multibyte | length & 127;
          write(padHex(2, byte), "hex");
          length = length >>> 7;
        }
        write(Buffer.from(await deflate(object)));
      }
      write("PACK");
      write("00000002", "hex");
      write(padHex(8, oids.length), "hex");
      for (const oid of oids) {
        const { type, object } = await _readObject({ fs: fs3, cache, gitdir, oid });
        await writeObject2({ write, object, stype: type });
      }
      const digest = hash.digest();
      outputStream.push(digest);
      return outputStream;
    }
    async function _packObjects({ fs: fs3, cache, gitdir, oids, write }) {
      const buffers = await _pack({ fs: fs3, cache, gitdir, oids });
      const packfile = Buffer.from(await collect(buffers));
      const packfileSha = packfile.slice(-20).toString("hex");
      const filename = `pack-${packfileSha}.pack`;
      if (write) {
        await fs3.write(join2(gitdir, `objects/pack/${filename}`), packfile);
        return { filename };
      }
      return {
        filename,
        packfile: new Uint8Array(packfile)
      };
    }
    async function packObjects({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      oids,
      write = false,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("oids", oids);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _packObjects({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          oids,
          write
        });
      } catch (err) {
        err.caller = "git.packObjects";
        throw err;
      }
    }
    async function pull({
      fs: _fs,
      http,
      onProgress,
      onMessage,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      url,
      remote,
      remoteRef,
      prune = false,
      pruneTags = false,
      fastForward: fastForward2 = true,
      fastForwardOnly = false,
      corsProxy,
      singleBranch,
      headers = {},
      author: _author,
      committer: _committer,
      signingKey,
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        const author = await normalizeAuthorObject({
          fs: fs3,
          gitdir: updatedGitdir,
          author: _author
        });
        if (!author) throw new MissingNameError("author");
        const committer = await normalizeCommitterObject({
          fs: fs3,
          gitdir: updatedGitdir,
          author,
          committer: _committer
        });
        if (!committer) throw new MissingNameError("committer");
        return await _pull({
          fs: fs3,
          cache,
          http,
          onProgress,
          onMessage,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          dir,
          gitdir: updatedGitdir,
          ref,
          url,
          remote,
          remoteRef,
          fastForward: fastForward2,
          fastForwardOnly,
          corsProxy,
          singleBranch,
          headers,
          author,
          committer,
          signingKey,
          prune,
          pruneTags
        });
      } catch (err) {
        err.caller = "git.pull";
        throw err;
      }
    }
    async function listCommitsAndTags({
      fs: fs3,
      cache,
      dir,
      gitdir = join2(dir, ".git"),
      start,
      finish
    }) {
      const shallows = await GitShallowManager.read({ fs: fs3, gitdir });
      const startingSet = /* @__PURE__ */ new Set();
      const finishingSet = /* @__PURE__ */ new Set();
      for (const ref of start) {
        startingSet.add(await GitRefManager.resolve({ fs: fs3, gitdir, ref }));
      }
      for (const ref of finish) {
        try {
          const oid = await GitRefManager.resolve({ fs: fs3, gitdir, ref });
          finishingSet.add(oid);
        } catch (err) {
        }
      }
      const visited = /* @__PURE__ */ new Set();
      async function walk2(oid) {
        visited.add(oid);
        const { type, object } = await _readObject({ fs: fs3, cache, gitdir, oid });
        if (type === "tag") {
          const tag2 = GitAnnotatedTag.from(object);
          const commit2 = tag2.headers().object;
          return walk2(commit2);
        }
        if (type !== "commit") {
          throw new ObjectTypeError(oid, type, "commit");
        }
        if (!shallows.has(oid)) {
          const commit2 = GitCommit.from(object);
          const parents = commit2.headers().parent;
          for (oid of parents) {
            if (!finishingSet.has(oid) && !visited.has(oid)) {
              await walk2(oid);
            }
          }
        }
      }
      for (const oid of startingSet) {
        await walk2(oid);
      }
      return visited;
    }
    async function listObjects({
      fs: fs3,
      cache,
      dir,
      gitdir = join2(dir, ".git"),
      oids
    }) {
      const visited = /* @__PURE__ */ new Set();
      async function walk2(oid) {
        if (visited.has(oid)) return;
        visited.add(oid);
        const { type, object } = await _readObject({ fs: fs3, cache, gitdir, oid });
        if (type === "tag") {
          const tag2 = GitAnnotatedTag.from(object);
          const obj = tag2.headers().object;
          await walk2(obj);
        } else if (type === "commit") {
          const commit2 = GitCommit.from(object);
          const tree = commit2.headers().tree;
          await walk2(tree);
        } else if (type === "tree") {
          const tree = GitTree.from(object);
          for (const entry of tree) {
            if (entry.type === "blob") {
              visited.add(entry.oid);
            }
            if (entry.type === "tree") {
              await walk2(entry.oid);
            }
          }
        }
      }
      for (const oid of oids) {
        await walk2(oid);
      }
      return visited;
    }
    async function parseReceivePackResponse(packfile) {
      const result = {};
      let response = "";
      const read = GitPktLine.streamReader(packfile);
      let line = await read();
      while (line !== true) {
        if (line !== null) response += line.toString("utf8") + "\n";
        line = await read();
      }
      const lines = response.toString("utf8").split("\n");
      line = lines.shift();
      if (!line.startsWith("unpack ")) {
        throw new ParseError('unpack ok" or "unpack [error message]', line);
      }
      result.ok = line === "unpack ok";
      if (!result.ok) {
        result.error = line.slice("unpack ".length);
      }
      result.refs = {};
      for (const line2 of lines) {
        if (line2.trim() === "") continue;
        const status2 = line2.slice(0, 2);
        const refAndMessage = line2.slice(3);
        let space = refAndMessage.indexOf(" ");
        if (space === -1) space = refAndMessage.length;
        const ref = refAndMessage.slice(0, space);
        const error = refAndMessage.slice(space + 1);
        result.refs[ref] = {
          ok: status2 === "ok",
          error
        };
      }
      return result;
    }
    async function writeReceivePackRequest({
      capabilities = [],
      triplets = []
    }) {
      const packstream = [];
      let capsFirstLine = `\0 ${capabilities.join(" ")}`;
      for (const trip of triplets) {
        packstream.push(
          GitPktLine.encode(
            `${trip.oldoid} ${trip.oid} ${trip.fullRef}${capsFirstLine}
`
          )
        );
        capsFirstLine = "";
      }
      packstream.push(GitPktLine.flush());
      return packstream;
    }
    async function _push({
      fs: fs3,
      cache,
      http,
      onProgress,
      onMessage,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      onPrePush,
      gitdir,
      ref: _ref,
      remoteRef: _remoteRef,
      remote,
      url: _url,
      force = false,
      delete: _delete = false,
      corsProxy,
      headers = {}
    }) {
      const ref = _ref || await _currentBranch({ fs: fs3, gitdir });
      if (typeof ref === "undefined") {
        throw new MissingParameterError("ref");
      }
      const config = await GitConfigManager.get({ fs: fs3, gitdir });
      remote = remote || await config.get(`branch.${ref}.pushRemote`) || await config.get("remote.pushDefault") || await config.get(`branch.${ref}.remote`) || "origin";
      const url = _url || await config.get(`remote.${remote}.pushurl`) || await config.get(`remote.${remote}.url`);
      if (typeof url === "undefined") {
        throw new MissingParameterError("remote OR url");
      }
      const remoteRef = _remoteRef || await config.get(`branch.${ref}.merge`);
      if (typeof url === "undefined") {
        throw new MissingParameterError("remoteRef");
      }
      if (corsProxy === void 0) {
        corsProxy = await config.get("http.corsProxy");
      }
      const fullRef = await GitRefManager.expand({ fs: fs3, gitdir, ref });
      const oid = _delete ? "0000000000000000000000000000000000000000" : await GitRefManager.resolve({ fs: fs3, gitdir, ref: fullRef });
      const GitRemoteHTTP2 = GitRemoteManager.getRemoteHelperFor({ url });
      const httpRemote = await GitRemoteHTTP2.discover({
        http,
        onAuth,
        onAuthSuccess,
        onAuthFailure,
        corsProxy,
        service: "git-receive-pack",
        url,
        headers,
        protocolVersion: 1
      });
      const auth = httpRemote.auth;
      let fullRemoteRef;
      if (!remoteRef) {
        fullRemoteRef = fullRef;
      } else {
        try {
          fullRemoteRef = await GitRefManager.expandAgainstMap({
            ref: remoteRef,
            map: httpRemote.refs
          });
        } catch (err) {
          if (err instanceof NotFoundError) {
            fullRemoteRef = remoteRef.startsWith("refs/") ? remoteRef : `refs/heads/${remoteRef}`;
          } else {
            throw err;
          }
        }
      }
      const oldoid = httpRemote.refs.get(fullRemoteRef) || "0000000000000000000000000000000000000000";
      if (onPrePush) {
        const hookCancel = await onPrePush({
          remote,
          url,
          localRef: { ref: _delete ? "(delete)" : fullRef, oid },
          remoteRef: { ref: fullRemoteRef, oid: oldoid }
        });
        if (!hookCancel) throw new UserCanceledError();
      }
      const thinPack = !httpRemote.capabilities.has("no-thin");
      let objects = /* @__PURE__ */ new Set();
      if (!_delete) {
        const finish = [...httpRemote.refs.values()];
        let skipObjects = /* @__PURE__ */ new Set();
        if (oldoid !== "0000000000000000000000000000000000000000") {
          const mergebase = await _findMergeBase({
            fs: fs3,
            cache,
            gitdir,
            oids: [oid, oldoid]
          });
          for (const oid2 of mergebase) finish.push(oid2);
          if (thinPack) {
            skipObjects = await listObjects({ fs: fs3, cache, gitdir, oids: mergebase });
          }
        }
        if (!finish.includes(oid)) {
          const commits = await listCommitsAndTags({
            fs: fs3,
            cache,
            gitdir,
            start: [oid],
            finish
          });
          objects = await listObjects({ fs: fs3, cache, gitdir, oids: commits });
        }
        if (thinPack) {
          try {
            const ref2 = await GitRefManager.resolve({
              fs: fs3,
              gitdir,
              ref: `refs/remotes/${remote}/HEAD`,
              depth: 2
            });
            const { oid: oid2 } = await GitRefManager.resolveAgainstMap({
              ref: ref2.replace(`refs/remotes/${remote}/`, ""),
              fullref: ref2,
              map: httpRemote.refs
            });
            const oids = [oid2];
            for (const oid3 of await listObjects({ fs: fs3, cache, gitdir, oids })) {
              skipObjects.add(oid3);
            }
          } catch (e) {
          }
          for (const oid2 of skipObjects) {
            objects.delete(oid2);
          }
        }
        if (oid === oldoid) force = true;
        if (!force) {
          if (fullRef.startsWith("refs/tags") && oldoid !== "0000000000000000000000000000000000000000") {
            throw new PushRejectedError("tag-exists");
          }
          if (oid !== "0000000000000000000000000000000000000000" && oldoid !== "0000000000000000000000000000000000000000" && !await _isDescendent({
            fs: fs3,
            cache,
            gitdir,
            oid,
            ancestor: oldoid,
            depth: -1
          })) {
            throw new PushRejectedError("not-fast-forward");
          }
        }
      }
      const capabilities = filterCapabilities(
        [...httpRemote.capabilities],
        ["report-status", "side-band-64k", `agent=${pkg.agent}`]
      );
      const packstream1 = await writeReceivePackRequest({
        capabilities,
        triplets: [{ oldoid, oid, fullRef: fullRemoteRef }]
      });
      const packstream2 = _delete ? [] : await _pack({
        fs: fs3,
        cache,
        gitdir,
        oids: [...objects]
      });
      const res = await GitRemoteHTTP2.connect({
        http,
        onProgress,
        corsProxy,
        service: "git-receive-pack",
        url,
        auth,
        headers,
        body: [...packstream1, ...packstream2]
      });
      const { packfile, progress } = await GitSideBand.demux(res.body);
      if (onMessage) {
        const lines = splitLines(progress);
        forAwait(lines, async (line) => {
          await onMessage(line);
        });
      }
      const result = await parseReceivePackResponse(packfile);
      if (res.headers) {
        result.headers = res.headers;
      }
      if (remote && result.ok && result.refs[fullRemoteRef].ok && !fullRef.startsWith("refs/tags")) {
        const ref2 = `refs/remotes/${remote}/${fullRemoteRef.replace(
          "refs/heads",
          ""
        )}`;
        if (_delete) {
          await GitRefManager.deleteRef({ fs: fs3, gitdir, ref: ref2 });
        } else {
          await GitRefManager.writeRef({ fs: fs3, gitdir, ref: ref2, value: oid });
        }
      }
      if (result.ok && Object.values(result.refs).every((result2) => result2.ok)) {
        return result;
      } else {
        const prettyDetails = Object.entries(result.refs).filter(([k, v]) => !v.ok).map(([k, v]) => `
  - ${k}: ${v.error}`).join("");
        throw new GitPushError(prettyDetails, result);
      }
    }
    async function push({
      fs: fs3,
      http,
      onProgress,
      onMessage,
      onAuth,
      onAuthSuccess,
      onAuthFailure,
      onPrePush,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      remoteRef,
      remote = "origin",
      url,
      force = false,
      delete: _delete = false,
      corsProxy,
      headers = {},
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("http", http);
        assertParameter("gitdir", gitdir);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _push({
          fs: fsp,
          cache,
          http,
          onProgress,
          onMessage,
          onAuth,
          onAuthSuccess,
          onAuthFailure,
          onPrePush,
          gitdir: updatedGitdir,
          ref,
          remoteRef,
          remote,
          url,
          force,
          delete: _delete,
          corsProxy,
          headers
        });
      } catch (err) {
        err.caller = "git.push";
        throw err;
      }
    }
    async function resolveBlob({ fs: fs3, cache, gitdir, oid }) {
      const { type, object } = await _readObject({ fs: fs3, cache, gitdir, oid });
      if (type === "tag") {
        oid = GitAnnotatedTag.from(object).parse().object;
        return resolveBlob({ fs: fs3, cache, gitdir, oid });
      }
      if (type !== "blob") {
        throw new ObjectTypeError(oid, type, "blob");
      }
      return { oid, blob: new Uint8Array(object) };
    }
    async function _readBlob({
      fs: fs3,
      cache,
      gitdir,
      oid,
      filepath = void 0
    }) {
      if (filepath !== void 0) {
        oid = await resolveFilepath({ fs: fs3, cache, gitdir, oid, filepath });
      }
      const blob = await resolveBlob({
        fs: fs3,
        cache,
        gitdir,
        oid
      });
      return blob;
    }
    async function readBlob2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      oid,
      filepath,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _readBlob({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          oid,
          filepath
        });
      } catch (err) {
        err.caller = "git.readBlob";
        throw err;
      }
    }
    async function readCommit2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      oid,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _readCommit({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          oid
        });
      } catch (err) {
        err.caller = "git.readCommit";
        throw err;
      }
    }
    async function _readNote({
      fs: fs3,
      cache,
      gitdir,
      ref = "refs/notes/commits",
      oid
    }) {
      const parent = await GitRefManager.resolve({ gitdir, fs: fs3, ref });
      const { blob } = await _readBlob({
        fs: fs3,
        cache,
        gitdir,
        oid: parent,
        filepath: oid
      });
      return blob;
    }
    async function readNote({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      ref = "refs/notes/commits",
      oid,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        assertParameter("oid", oid);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _readNote({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          ref,
          oid
        });
      } catch (err) {
        err.caller = "git.readNote";
        throw err;
      }
    }
    async function readObject2({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      oid,
      format = "parsed",
      filepath = void 0,
      encoding = void 0,
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        if (filepath !== void 0) {
          oid = await resolveFilepath({
            fs: fs3,
            cache,
            gitdir: updatedGitdir,
            oid,
            filepath
          });
        }
        const _format = format === "parsed" ? "content" : format;
        const result = await _readObject({
          fs: fs3,
          cache,
          gitdir: updatedGitdir,
          oid,
          format: _format
        });
        result.oid = oid;
        if (format === "parsed") {
          result.format = "parsed";
          switch (result.type) {
            case "commit":
              result.object = GitCommit.from(result.object).parse();
              break;
            case "tree":
              result.object = GitTree.from(result.object).entries();
              break;
            case "blob":
              if (encoding) {
                result.object = result.object.toString(encoding);
              } else {
                result.object = new Uint8Array(result.object);
                result.format = "content";
              }
              break;
            case "tag":
              result.object = GitAnnotatedTag.from(result.object).parse();
              break;
            default:
              throw new ObjectTypeError(
                result.oid,
                result.type,
                "blob|commit|tag|tree"
              );
          }
        } else if (result.format === "deflated" || result.format === "wrapped") {
          result.type = result.format;
        }
        return result;
      } catch (err) {
        err.caller = "git.readObject";
        throw err;
      }
    }
    async function _readTag({ fs: fs3, cache, gitdir, oid }) {
      const { type, object } = await _readObject({
        fs: fs3,
        cache,
        gitdir,
        oid,
        format: "content"
      });
      if (type !== "tag") {
        throw new ObjectTypeError(oid, type, "tag");
      }
      const tag2 = GitAnnotatedTag.from(object);
      const result = {
        oid,
        tag: tag2.parse(),
        payload: tag2.payload()
      };
      return result;
    }
    async function readTag({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      oid,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _readTag({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          oid
        });
      } catch (err) {
        err.caller = "git.readTag";
        throw err;
      }
    }
    async function readTree2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      oid,
      filepath = void 0,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _readTree({
          fs: fsp,
          cache,
          gitdir: updatedGitdir,
          oid,
          filepath
        });
      } catch (err) {
        err.caller = "git.readTree";
        throw err;
      }
    }
    async function remove({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      filepath,
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("filepath", filepath);
        const fsp = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        await GitIndexManager.acquire(
          { fs: fsp, gitdir: updatedGitdir, cache },
          async function(index2) {
            index2.delete({ filepath });
          }
        );
      } catch (err) {
        err.caller = "git.remove";
        throw err;
      }
    }
    async function _removeNote({
      fs: fs3,
      cache,
      onSign,
      gitdir,
      ref = "refs/notes/commits",
      oid,
      author,
      committer,
      signingKey
    }) {
      let parent;
      try {
        parent = await GitRefManager.resolve({ gitdir, fs: fs3, ref });
      } catch (err) {
        if (!(err instanceof NotFoundError)) {
          throw err;
        }
      }
      const result = await _readTree({
        fs: fs3,
        cache,
        gitdir,
        oid: parent || "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
      });
      let tree = result.tree;
      tree = tree.filter((entry) => entry.path !== oid);
      const treeOid = await _writeTree({
        fs: fs3,
        gitdir,
        tree
      });
      const commitOid = await _commit({
        fs: fs3,
        cache,
        onSign,
        gitdir,
        ref,
        tree: treeOid,
        parent: parent && [parent],
        message: `Note removed by 'isomorphic-git removeNote'
`,
        author,
        committer,
        signingKey
      });
      return commitOid;
    }
    async function removeNote({
      fs: _fs,
      onSign,
      dir,
      gitdir = join2(dir, ".git"),
      ref = "refs/notes/commits",
      oid,
      author: _author,
      committer: _committer,
      signingKey,
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("oid", oid);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        const author = await normalizeAuthorObject({
          fs: fs3,
          gitdir: updatedGitdir,
          author: _author
        });
        if (!author) throw new MissingNameError("author");
        const committer = await normalizeCommitterObject({
          fs: fs3,
          gitdir: updatedGitdir,
          author,
          committer: _committer
        });
        if (!committer) throw new MissingNameError("committer");
        return await _removeNote({
          fs: fs3,
          cache,
          onSign,
          gitdir: updatedGitdir,
          ref,
          oid,
          author,
          committer,
          signingKey
        });
      } catch (err) {
        err.caller = "git.removeNote";
        throw err;
      }
    }
    async function _renameBranch({
      fs: fs3,
      gitdir,
      oldref,
      ref,
      checkout: checkout2 = false
    }) {
      if (!isValidRef(ref, true)) {
        throw new InvalidRefNameError(ref, cleanGitRef.clean(ref));
      }
      if (!isValidRef(oldref, true)) {
        throw new InvalidRefNameError(oldref, cleanGitRef.clean(oldref));
      }
      const fulloldref = `refs/heads/${oldref}`;
      const fullnewref = `refs/heads/${ref}`;
      const newexist = await GitRefManager.exists({ fs: fs3, gitdir, ref: fullnewref });
      if (newexist) {
        throw new AlreadyExistsError("branch", ref, false);
      }
      const value = await GitRefManager.resolve({
        fs: fs3,
        gitdir,
        ref: fulloldref,
        depth: 1
      });
      await GitRefManager.writeRef({ fs: fs3, gitdir, ref: fullnewref, value });
      await GitRefManager.deleteRef({ fs: fs3, gitdir, ref: fulloldref });
      const fullCurrentBranchRef = await _currentBranch({
        fs: fs3,
        gitdir,
        fullname: true
      });
      const isCurrentBranch = fullCurrentBranchRef === fulloldref;
      if (checkout2 || isCurrentBranch) {
        await GitRefManager.writeSymbolicRef({
          fs: fs3,
          gitdir,
          ref: "HEAD",
          value: fullnewref
        });
      }
    }
    async function renameBranch({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      oldref,
      checkout: checkout2 = false
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        assertParameter("oldref", oldref);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _renameBranch({
          fs: fsp,
          gitdir: updatedGitdir,
          ref,
          oldref,
          checkout: checkout2
        });
      } catch (err) {
        err.caller = "git.renameBranch";
        throw err;
      }
    }
    async function hashObject$1({ gitdir, type, object }) {
      return shasum(GitObject.wrap({ type, object }));
    }
    async function resetIndex({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      filepath,
      ref,
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("filepath", filepath);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        let oid;
        let workdirOid;
        try {
          oid = await GitRefManager.resolve({
            fs: fs3,
            gitdir: updatedGitdir,
            ref: ref || "HEAD"
          });
        } catch (e) {
          if (ref) {
            throw e;
          }
        }
        if (oid) {
          try {
            oid = await resolveFilepath({
              fs: fs3,
              cache,
              gitdir: updatedGitdir,
              oid,
              filepath
            });
          } catch (e) {
            oid = null;
          }
        }
        let stats = {
          ctime: /* @__PURE__ */ new Date(0),
          mtime: /* @__PURE__ */ new Date(0),
          dev: 0,
          ino: 0,
          mode: 0,
          uid: 0,
          gid: 0,
          size: 0
        };
        const object = dir && await fs3.read(join2(dir, filepath));
        if (object) {
          workdirOid = await hashObject$1({
            gitdir: updatedGitdir,
            type: "blob",
            object
          });
          if (oid === workdirOid) {
            stats = await fs3.lstat(join2(dir, filepath));
          }
        }
        await GitIndexManager.acquire(
          { fs: fs3, gitdir: updatedGitdir, cache },
          async function(index2) {
            index2.delete({ filepath });
            if (oid) {
              index2.insert({ filepath, stats, oid });
            }
          }
        );
      } catch (err) {
        err.caller = "git.reset";
        throw err;
      }
    }
    async function resolveRef2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      depth
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        const oid = await GitRefManager.resolve({
          fs: fsp,
          gitdir: updatedGitdir,
          ref,
          depth
        });
        return oid;
      } catch (err) {
        err.caller = "git.resolveRef";
        throw err;
      }
    }
    async function setConfig({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      path: path3,
      value,
      append = false
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("path", path3);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        const config = await GitConfigManager.get({ fs: fs3, gitdir: updatedGitdir });
        if (append) {
          await config.append(path3, value);
        } else {
          await config.set(path3, value);
        }
        await GitConfigManager.save({ fs: fs3, gitdir: updatedGitdir, config });
      } catch (err) {
        err.caller = "git.setConfig";
        throw err;
      }
    }
    async function _writeCommit({ fs: fs3, gitdir, commit: commit2 }) {
      const object = GitCommit.from(commit2).toObject();
      const oid = await _writeObject({
        fs: fs3,
        gitdir,
        type: "commit",
        object,
        format: "content"
      });
      return oid;
    }
    var GitRefStash = class _GitRefStash {
      // constructor removed
      static get timezoneOffsetForRefLogEntry() {
        const offsetMinutes = (/* @__PURE__ */ new Date()).getTimezoneOffset();
        const offsetHours = Math.abs(Math.floor(offsetMinutes / 60));
        const offsetMinutesFormatted = Math.abs(offsetMinutes % 60).toString().padStart(2, "0");
        const sign = offsetMinutes > 0 ? "-" : "+";
        return `${sign}${offsetHours.toString().padStart(2, "0")}${offsetMinutesFormatted}`;
      }
      static createStashReflogEntry(author, stashCommit, message) {
        const nameNoSpace = author.name.replace(/\s/g, "");
        const z40 = "0000000000000000000000000000000000000000";
        const timestamp = Math.floor(Date.now() / 1e3);
        const timezoneOffset = _GitRefStash.timezoneOffsetForRefLogEntry;
        return `${z40} ${stashCommit} ${nameNoSpace} ${author.email} ${timestamp} ${timezoneOffset}	${message}
`;
      }
      static getStashReflogEntry(reflogString, parsed = false) {
        const reflogLines = reflogString.split("\n");
        const entries = reflogLines.filter((l) => l).reverse().map(
          (line, idx) => parsed ? `stash@{${idx}}: ${line.split("	")[1]}` : line
        );
        return entries;
      }
    };
    var GitStashManager = class _GitStashManager {
      /**
       * Creates an instance of GitStashManager.
       *
       * @param {Object} args
       * @param {FSClient} args.fs - A file system implementation.
       * @param {string} args.dir - The working directory.
       * @param {string}[args.gitdir=join(dir, '.git')] - [required] The [git directory](dir-vs-gitdir.md) path
       */
      constructor({ fs: fs3, dir, gitdir = join2(dir, ".git") }) {
        Object.assign(this, {
          fs: fs3,
          dir,
          gitdir,
          _author: null
        });
      }
      /**
       * Gets the reference name for the stash.
       *
       * @returns {string} - The stash reference name.
       */
      static get refStash() {
        return "refs/stash";
      }
      /**
       * Gets the reference name for the stash reflogs.
       *
       * @returns {string} - The stash reflogs reference name.
       */
      static get refLogsStash() {
        return "logs/refs/stash";
      }
      /**
       * Gets the file path for the stash reference.
       *
       * @returns {string} - The file path for the stash reference.
       */
      get refStashPath() {
        return join2(this.gitdir, _GitStashManager.refStash);
      }
      /**
       * Gets the file path for the stash reflogs.
       *
       * @returns {string} - The file path for the stash reflogs.
       */
      get refLogsStashPath() {
        return join2(this.gitdir, _GitStashManager.refLogsStash);
      }
      /**
       * Retrieves the author information for the stash.
       *
       * @returns {Promise<Object>} - The author object.
       * @throws {MissingNameError} - If the author name is missing.
       */
      async getAuthor() {
        if (!this._author) {
          this._author = await normalizeAuthorObject({
            fs: this.fs,
            gitdir: this.gitdir,
            author: {}
          });
          if (!this._author) throw new MissingNameError("author");
        }
        return this._author;
      }
      /**
       * Gets the SHA of a stash entry by its index.
       *
       * @param {number} refIdx - The index of the stash entry.
       * @param {string[]} [stashEntries] - Optional preloaded stash entries.
       * @returns {Promise<string|null>} - The SHA of the stash entry or `null` if not found.
       */
      async getStashSHA(refIdx, stashEntries) {
        if (!await this.fs.exists(this.refStashPath)) {
          return null;
        }
        const entries = stashEntries || await this.readStashReflogs({ parsed: false });
        return entries[refIdx].split(" ")[1];
      }
      /**
       * Writes a stash commit to the repository.
       *
       * @param {Object} args
       * @param {string} args.message - The commit message.
       * @param {string} args.tree - The tree object ID.
       * @param {string[]} args.parent - The parent commit object IDs.
       * @returns {Promise<string>} - The object ID of the written commit.
       */
      async writeStashCommit({ message, tree, parent }) {
        return _writeCommit({
          fs: this.fs,
          gitdir: this.gitdir,
          commit: {
            message,
            tree,
            parent,
            author: await this.getAuthor(),
            committer: await this.getAuthor()
          }
        });
      }
      /**
       * Reads a stash commit by its index.
       *
       * @param {number} refIdx - The index of the stash entry.
       * @returns {Promise<Object>} - The stash commit object.
       * @throws {InvalidRefNameError} - If the index is invalid.
       */
      async readStashCommit(refIdx) {
        const stashEntries = await this.readStashReflogs({ parsed: false });
        if (refIdx !== 0) {
          if (refIdx < 0 || refIdx > stashEntries.length - 1) {
            throw new InvalidRefNameError(
              `stash@${refIdx}`,
              "number that is in range of [0, num of stash pushed]"
            );
          }
        }
        const stashSHA = await this.getStashSHA(refIdx, stashEntries);
        if (!stashSHA) {
          return {};
        }
        return _readCommit({
          fs: this.fs,
          cache: {},
          gitdir: this.gitdir,
          oid: stashSHA
        });
      }
      /**
       * Writes a stash reference to the repository.
       *
       * @param {string} stashCommit - The object ID of the stash commit.
       * @returns {Promise<void>}
       */
      async writeStashRef(stashCommit) {
        return GitRefManager.writeRef({
          fs: this.fs,
          gitdir: this.gitdir,
          ref: _GitStashManager.refStash,
          value: stashCommit
        });
      }
      /**
       * Writes a reflog entry for a stash commit.
       *
       * @param {Object} args
       * @param {string} args.stashCommit - The object ID of the stash commit.
       * @param {string} args.message - The reflog message.
       * @returns {Promise<void>}
       */
      async writeStashReflogEntry({ stashCommit, message }) {
        const author = await this.getAuthor();
        const entry = GitRefStash.createStashReflogEntry(
          author,
          stashCommit,
          message
        );
        const filepath = this.refLogsStashPath;
        await acquireLock$1({ filepath, entry }, async () => {
          const appendTo = await this.fs.exists(filepath) ? await this.fs.read(filepath, "utf8") : "";
          await this.fs.write(filepath, appendTo + entry, "utf8");
        });
      }
      /**
       * Reads the stash reflogs.
       *
       * @param {Object} args
       * @param {boolean} [args.parsed=false] - Whether to parse the reflog entries.
       * @returns {Promise<string[]|Object[]>} - The reflog entries as strings or parsed objects.
       */
      async readStashReflogs({ parsed = false }) {
        if (!await this.fs.exists(this.refLogsStashPath)) {
          return [];
        }
        const reflogString = await this.fs.read(this.refLogsStashPath, "utf8");
        return GitRefStash.getStashReflogEntry(reflogString, parsed);
      }
    };
    async function _createStashCommit({ fs: fs3, dir, gitdir, message = "" }) {
      const stashMgr = new GitStashManager({ fs: fs3, dir, gitdir });
      await stashMgr.getAuthor();
      const branch2 = await _currentBranch({
        fs: fs3,
        gitdir,
        fullname: false
      });
      const headCommit = await GitRefManager.resolve({
        fs: fs3,
        gitdir,
        ref: "HEAD"
      });
      const headCommitObj = await readCommit2({ fs: fs3, dir, gitdir, oid: headCommit });
      const headMsg = headCommitObj.commit.message;
      const stashCommitParents = [headCommit];
      let stashCommitTree = null;
      let workDirCompareBase = TREE({ ref: "HEAD" });
      const indexTree = await writeTreeChanges({
        fs: fs3,
        dir,
        gitdir,
        treePair: [TREE({ ref: "HEAD" }), "stage"]
      });
      if (indexTree) {
        const stashCommitOne = await stashMgr.writeStashCommit({
          message: `stash-Index: WIP on ${branch2} - ${(/* @__PURE__ */ new Date()).toISOString()}`,
          tree: indexTree,
          parent: stashCommitParents
        });
        stashCommitParents.push(stashCommitOne);
        stashCommitTree = indexTree;
        workDirCompareBase = STAGE();
      }
      const workingTree = await writeTreeChanges({
        fs: fs3,
        dir,
        gitdir,
        treePair: [workDirCompareBase, "workdir"]
      });
      if (workingTree) {
        const workingHeadCommit = await stashMgr.writeStashCommit({
          message: `stash-WorkDir: WIP on ${branch2} - ${(/* @__PURE__ */ new Date()).toISOString()}`,
          tree: workingTree,
          parent: [stashCommitParents[stashCommitParents.length - 1]]
        });
        stashCommitParents.push(workingHeadCommit);
        stashCommitTree = workingTree;
      }
      if (!stashCommitTree || !indexTree && !workingTree) {
        throw new NotFoundError("changes, nothing to stash");
      }
      const stashMsg = (message.trim() || `WIP on ${branch2}`) + `: ${headCommit.substring(0, 7)} ${headMsg}`;
      const stashCommit = await stashMgr.writeStashCommit({
        message: stashMsg,
        tree: stashCommitTree,
        parent: stashCommitParents
      });
      return { stashCommit, stashMsg, branch: branch2, stashMgr };
    }
    async function _stashPush({ fs: fs3, dir, gitdir, message = "" }) {
      const { stashCommit, stashMsg, branch: branch2, stashMgr } = await _createStashCommit({
        fs: fs3,
        dir,
        gitdir,
        message
      });
      await stashMgr.writeStashRef(stashCommit);
      await stashMgr.writeStashReflogEntry({
        stashCommit,
        message: stashMsg
      });
      await checkout({
        fs: fs3,
        dir,
        gitdir,
        ref: branch2,
        track: false,
        force: true
        // force checkout to discard changes
      });
      return stashCommit;
    }
    async function _stashCreate({ fs: fs3, dir, gitdir, message = "" }) {
      const { stashCommit } = await _createStashCommit({
        fs: fs3,
        dir,
        gitdir,
        message
      });
      return stashCommit;
    }
    async function _stashApply({ fs: fs3, dir, gitdir, refIdx = 0 }) {
      const stashMgr = new GitStashManager({ fs: fs3, dir, gitdir });
      const stashCommit = await stashMgr.readStashCommit(refIdx);
      const { parent: stashParents = null } = stashCommit.commit ? stashCommit.commit : {};
      if (!stashParents || !Array.isArray(stashParents)) {
        return;
      }
      for (let i = 0; i < stashParents.length - 1; i++) {
        const applyingCommit = await _readCommit({
          fs: fs3,
          cache: {},
          gitdir,
          oid: stashParents[i + 1]
        });
        const wasStaged = applyingCommit.commit.message.startsWith("stash-Index");
        await applyTreeChanges({
          fs: fs3,
          dir,
          gitdir,
          stashCommit: stashParents[i + 1],
          parentCommit: stashParents[i],
          wasStaged
        });
      }
    }
    async function _stashDrop({ fs: fs3, dir, gitdir, refIdx = 0 }) {
      const stashMgr = new GitStashManager({ fs: fs3, dir, gitdir });
      const stashCommit = await stashMgr.readStashCommit(refIdx);
      if (!stashCommit.commit) {
        return;
      }
      const stashRefPath = stashMgr.refStashPath;
      await acquireLock$1(stashRefPath, async () => {
        if (await fs3.exists(stashRefPath)) {
          await fs3.rm(stashRefPath);
        }
      });
      const reflogEntries = await stashMgr.readStashReflogs({ parsed: false });
      if (!reflogEntries.length) {
        return;
      }
      reflogEntries.splice(refIdx, 1);
      const stashReflogPath = stashMgr.refLogsStashPath;
      await acquireLock$1({ reflogEntries, stashReflogPath, stashMgr }, async () => {
        if (reflogEntries.length) {
          await fs3.write(
            stashReflogPath,
            reflogEntries.reverse().join("\n") + "\n",
            "utf8"
          );
          const lastStashCommit = reflogEntries[reflogEntries.length - 1].split(" ")[1];
          await stashMgr.writeStashRef(lastStashCommit);
        } else {
          await fs3.rm(stashReflogPath);
        }
      });
    }
    async function _stashList({ fs: fs3, dir, gitdir }) {
      const stashMgr = new GitStashManager({ fs: fs3, dir, gitdir });
      return stashMgr.readStashReflogs({ parsed: true });
    }
    async function _stashClear({ fs: fs3, dir, gitdir }) {
      const stashMgr = new GitStashManager({ fs: fs3, dir, gitdir });
      const stashRefPath = [stashMgr.refStashPath, stashMgr.refLogsStashPath];
      await acquireLock$1(stashRefPath, async () => {
        await Promise.all(
          stashRefPath.map(async (path3) => {
            if (await fs3.exists(path3)) {
              return fs3.rm(path3);
            }
          })
        );
      });
    }
    async function _stashPop({ fs: fs3, dir, gitdir, refIdx = 0 }) {
      await _stashApply({ fs: fs3, dir, gitdir, refIdx });
      await _stashDrop({ fs: fs3, dir, gitdir, refIdx });
    }
    async function stash({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      op = "push",
      message = "",
      refIdx = 0
    }) {
      assertParameter("fs", fs3);
      assertParameter("dir", dir);
      assertParameter("gitdir", gitdir);
      assertParameter("op", op);
      const stashMap = {
        push: _stashPush,
        apply: _stashApply,
        drop: _stashDrop,
        list: _stashList,
        clear: _stashClear,
        pop: _stashPop,
        create: _stashCreate
      };
      const opsNeedRefIdx = ["apply", "drop", "pop"];
      try {
        const _fs = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp: _fs, dotgit: gitdir });
        const folders = ["refs", "logs", "logs/refs"];
        folders.map((f) => join2(updatedGitdir, f)).forEach(async (folder) => {
          if (!await _fs.exists(folder)) {
            await _fs.mkdir(folder);
          }
        });
        const opFunc = stashMap[op];
        if (opFunc) {
          if (opsNeedRefIdx.includes(op) && refIdx < 0) {
            throw new InvalidRefNameError(
              `stash@${refIdx}`,
              "number that is in range of [0, num of stash pushed]"
            );
          }
          return await opFunc({
            fs: _fs,
            dir,
            gitdir: updatedGitdir,
            message,
            refIdx
          });
        }
        throw new Error(`To be implemented: ${op}`);
      } catch (err) {
        err.caller = "git.stash";
        throw err;
      }
    }
    async function status({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      filepath,
      cache = {}
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("filepath", filepath);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        const ignored = await GitIgnoreManager.isIgnored({
          fs: fs3,
          gitdir: updatedGitdir,
          dir,
          filepath
        });
        if (ignored) {
          return "ignored";
        }
        const headTree = await getHeadTree({ fs: fs3, cache, gitdir: updatedGitdir });
        const treeOid = await getOidAtPath({
          fs: fs3,
          cache,
          gitdir: updatedGitdir,
          tree: headTree,
          path: filepath
        });
        const indexEntry = await GitIndexManager.acquire(
          { fs: fs3, gitdir: updatedGitdir, cache },
          async function(index2) {
            for (const entry of index2) {
              if (entry.path === filepath) return entry;
            }
            return null;
          }
        );
        const stats = await fs3.lstat(join2(dir, filepath));
        const H = treeOid !== null;
        const I = indexEntry !== null;
        const W = stats !== null;
        const getWorkdirOid = async () => {
          if (I && !compareStats(indexEntry, stats)) {
            return indexEntry.oid;
          } else {
            const object = await fs3.read(join2(dir, filepath));
            const workdirOid = await hashObject$1({
              gitdir: updatedGitdir,
              type: "blob",
              object
            });
            if (I && indexEntry.oid === workdirOid) {
              if (stats.size !== -1) {
                GitIndexManager.acquire(
                  { fs: fs3, gitdir: updatedGitdir, cache },
                  async function(index2) {
                    index2.insert({ filepath, stats, oid: workdirOid });
                  }
                );
              }
            }
            return workdirOid;
          }
        };
        if (!H && !W && !I) return "absent";
        if (!H && !W && I) return "*absent";
        if (!H && W && !I) return "*added";
        if (!H && W && I) {
          const workdirOid = await getWorkdirOid();
          return workdirOid === indexEntry.oid ? "added" : "*added";
        }
        if (H && !W && !I) return "deleted";
        if (H && !W && I) {
          return treeOid === indexEntry.oid ? "*deleted" : "*deleted";
        }
        if (H && W && !I) {
          const workdirOid = await getWorkdirOid();
          return workdirOid === treeOid ? "*undeleted" : "*undeletemodified";
        }
        if (H && W && I) {
          const workdirOid = await getWorkdirOid();
          if (workdirOid === treeOid) {
            return workdirOid === indexEntry.oid ? "unmodified" : "*unmodified";
          } else {
            return workdirOid === indexEntry.oid ? "modified" : "*modified";
          }
        }
      } catch (err) {
        err.caller = "git.status";
        throw err;
      }
    }
    async function getOidAtPath({ fs: fs3, cache, gitdir: updatedGitdir, tree, path: path3 }) {
      if (typeof path3 === "string") path3 = path3.split("/");
      const dirname2 = path3.shift();
      for (const entry of tree) {
        if (entry.path === dirname2) {
          if (path3.length === 0) {
            return entry.oid;
          }
          const { type, object } = await _readObject({
            fs: fs3,
            cache,
            gitdir: updatedGitdir,
            oid: entry.oid
          });
          if (type === "tree") {
            const tree2 = GitTree.from(object);
            return getOidAtPath({ fs: fs3, cache, gitdir: updatedGitdir, tree: tree2, path: path3 });
          }
          if (type === "blob") {
            throw new ObjectTypeError(entry.oid, type, "blob", path3.join("/"));
          }
        }
      }
      return null;
    }
    async function getHeadTree({ fs: fs3, cache, gitdir: updatedGitdir }) {
      let oid;
      try {
        oid = await GitRefManager.resolve({
          fs: fs3,
          gitdir: updatedGitdir,
          ref: "HEAD"
        });
      } catch (e) {
        if (e instanceof NotFoundError) {
          return [];
        }
      }
      const { tree } = await _readTree({ fs: fs3, cache, gitdir: updatedGitdir, oid });
      return tree;
    }
    async function statusMatrix({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      ref = "HEAD",
      filepaths = ["."],
      filter,
      cache = {},
      ignored: shouldIgnore = false
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        return await _walk({
          fs: fs3,
          cache,
          dir,
          gitdir: updatedGitdir,
          trees: [TREE({ ref }), WORKDIR(), STAGE()],
          map: async function(filepath, [head, workdir, stage]) {
            if (!head && !stage && workdir) {
              if (!shouldIgnore) {
                const isIgnored2 = await GitIgnoreManager.isIgnored({
                  fs: fs3,
                  dir,
                  filepath
                });
                if (isIgnored2) {
                  return null;
                }
              }
            }
            if (!filepaths.some((base) => worthWalking(filepath, base))) {
              return null;
            }
            if (filter) {
              if (!filter(filepath)) return;
            }
            const [headType, workdirType, stageType] = await Promise.all([
              head && head.type(),
              workdir && workdir.type(),
              stage && stage.type()
            ]);
            const isBlob = [headType, workdirType, stageType].includes("blob");
            if ((headType === "tree" || headType === "special") && !isBlob) return;
            if (headType === "commit") return null;
            if ((workdirType === "tree" || workdirType === "special") && !isBlob)
              return;
            if (stageType === "commit") return null;
            if ((stageType === "tree" || stageType === "special") && !isBlob) return;
            const headOid = headType === "blob" ? await head.oid() : void 0;
            const stageOid = stageType === "blob" ? await stage.oid() : void 0;
            let workdirOid;
            if (headType !== "blob" && workdirType === "blob" && stageType !== "blob") {
              workdirOid = "42";
            } else if (workdirType === "blob") {
              workdirOid = await workdir.oid();
            }
            const entry = [void 0, headOid, workdirOid, stageOid];
            const result = entry.map((value) => entry.indexOf(value));
            result.shift();
            return [filepath, ...result];
          }
        });
      } catch (err) {
        err.caller = "git.statusMatrix";
        throw err;
      }
    }
    async function tag({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      object,
      force = false
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        const fs3 = new FileSystem(_fs);
        if (ref === void 0) {
          throw new MissingParameterError("ref");
        }
        ref = ref.startsWith("refs/tags/") ? ref : `refs/tags/${ref}`;
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        const value = await GitRefManager.resolve({
          fs: fs3,
          gitdir: updatedGitdir,
          ref: object || "HEAD"
        });
        if (!force && await GitRefManager.exists({ fs: fs3, gitdir: updatedGitdir, ref })) {
          throw new AlreadyExistsError("tag", ref);
        }
        await GitRefManager.writeRef({ fs: fs3, gitdir: updatedGitdir, ref, value });
      } catch (err) {
        err.caller = "git.tag";
        throw err;
      }
    }
    async function updateIndex$1({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      cache = {},
      filepath,
      oid,
      mode,
      add: add2,
      remove: remove2,
      force
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("filepath", filepath);
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        if (remove2) {
          return await GitIndexManager.acquire(
            { fs: fs3, gitdir: updatedGitdir, cache },
            async function(index2) {
              if (!force) {
                const fileStats2 = await fs3.lstat(join2(dir, filepath));
                if (fileStats2) {
                  if (fileStats2.isDirectory()) {
                    throw new InvalidFilepathError("directory");
                  }
                  return;
                }
              }
              if (index2.has({ filepath })) {
                index2.delete({
                  filepath
                });
              }
            }
          );
        }
        let fileStats;
        if (!oid) {
          fileStats = await fs3.lstat(join2(dir, filepath));
          if (!fileStats) {
            throw new NotFoundError(
              `file at "${filepath}" on disk and "remove" not set`
            );
          }
          if (fileStats.isDirectory()) {
            throw new InvalidFilepathError("directory");
          }
        }
        return await GitIndexManager.acquire(
          { fs: fs3, gitdir: updatedGitdir, cache },
          async function(index2) {
            if (!add2 && !index2.has({ filepath })) {
              throw new NotFoundError(
                `file at "${filepath}" in index and "add" not set`
              );
            }
            let stats;
            if (!oid) {
              stats = fileStats;
              const object = stats.isSymbolicLink() ? await fs3.readlink(join2(dir, filepath)) : await fs3.read(join2(dir, filepath));
              oid = await _writeObject({
                fs: fs3,
                gitdir: updatedGitdir,
                type: "blob",
                format: "content",
                object
              });
            } else {
              stats = {
                ctime: /* @__PURE__ */ new Date(0),
                mtime: /* @__PURE__ */ new Date(0),
                dev: 0,
                ino: 0,
                mode,
                uid: 0,
                gid: 0,
                size: 0
              };
            }
            index2.insert({
              filepath,
              oid,
              stats
            });
            return oid;
          }
        );
      } catch (err) {
        err.caller = "git.updateIndex";
        throw err;
      }
    }
    function version() {
      try {
        return pkg.version;
      } catch (err) {
        err.caller = "git.version";
        throw err;
      }
    }
    async function walk({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      trees,
      map,
      reduce,
      iterate,
      cache = {}
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("trees", trees);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _walk({
          fs: fsp,
          cache,
          dir,
          gitdir: updatedGitdir,
          trees,
          map,
          reduce,
          iterate
        });
      } catch (err) {
        err.caller = "git.walk";
        throw err;
      }
    }
    async function writeBlob2({ fs: fs3, dir, gitdir = join2(dir, ".git"), blob }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("blob", blob);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _writeObject({
          fs: fsp,
          gitdir: updatedGitdir,
          type: "blob",
          object: blob,
          format: "content"
        });
      } catch (err) {
        err.caller = "git.writeBlob";
        throw err;
      }
    }
    async function writeCommit2({
      fs: fs3,
      dir,
      gitdir = join2(dir, ".git"),
      commit: commit2
    }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("commit", commit2);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _writeCommit({
          fs: fsp,
          gitdir: updatedGitdir,
          commit: commit2
        });
      } catch (err) {
        err.caller = "git.writeCommit";
        throw err;
      }
    }
    async function writeObject({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      type,
      object,
      format = "parsed",
      oid,
      encoding = void 0
    }) {
      try {
        const fs3 = new FileSystem(_fs);
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        if (format === "parsed") {
          switch (type) {
            case "commit":
              object = GitCommit.from(object).toObject();
              break;
            case "tree":
              object = GitTree.from(object).toObject();
              break;
            case "blob":
              object = Buffer.from(object, encoding);
              break;
            case "tag":
              object = GitAnnotatedTag.from(object).toObject();
              break;
            default:
              throw new ObjectTypeError(oid || "", type, "blob|commit|tag|tree");
          }
          format = "content";
        }
        oid = await _writeObject({
          fs: fs3,
          gitdir: updatedGitdir,
          type,
          object,
          oid,
          format
        });
        return oid;
      } catch (err) {
        err.caller = "git.writeObject";
        throw err;
      }
    }
    async function writeRef2({
      fs: _fs,
      dir,
      gitdir = join2(dir, ".git"),
      ref,
      value,
      force = false,
      symbolic = false
    }) {
      try {
        assertParameter("fs", _fs);
        assertParameter("gitdir", gitdir);
        assertParameter("ref", ref);
        assertParameter("value", value);
        const fs3 = new FileSystem(_fs);
        if (!isValidRef(ref, true)) {
          throw new InvalidRefNameError(ref, cleanGitRef.clean(ref));
        }
        const updatedGitdir = await discoverGitdir({ fsp: fs3, dotgit: gitdir });
        if (!force && await GitRefManager.exists({ fs: fs3, gitdir: updatedGitdir, ref })) {
          throw new AlreadyExistsError("ref", ref);
        }
        if (symbolic) {
          await GitRefManager.writeSymbolicRef({
            fs: fs3,
            gitdir: updatedGitdir,
            ref,
            value
          });
        } else {
          value = await GitRefManager.resolve({
            fs: fs3,
            gitdir: updatedGitdir,
            ref: value
          });
          await GitRefManager.writeRef({
            fs: fs3,
            gitdir: updatedGitdir,
            ref,
            value
          });
        }
      } catch (err) {
        err.caller = "git.writeRef";
        throw err;
      }
    }
    async function _writeTag({ fs: fs3, gitdir, tag: tag2 }) {
      const object = GitAnnotatedTag.from(tag2).toObject();
      const oid = await _writeObject({
        fs: fs3,
        gitdir,
        type: "tag",
        object,
        format: "content"
      });
      return oid;
    }
    async function writeTag({ fs: fs3, dir, gitdir = join2(dir, ".git"), tag: tag2 }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("tag", tag2);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _writeTag({
          fs: fsp,
          gitdir: updatedGitdir,
          tag: tag2
        });
      } catch (err) {
        err.caller = "git.writeTag";
        throw err;
      }
    }
    async function writeTree2({ fs: fs3, dir, gitdir = join2(dir, ".git"), tree }) {
      try {
        assertParameter("fs", fs3);
        assertParameter("gitdir", gitdir);
        assertParameter("tree", tree);
        const fsp = new FileSystem(fs3);
        const updatedGitdir = await discoverGitdir({ fsp, dotgit: gitdir });
        return await _writeTree({
          fs: fsp,
          gitdir: updatedGitdir,
          tree
        });
      } catch (err) {
        err.caller = "git.writeTree";
        throw err;
      }
    }
    var index = {
      Errors,
      STAGE,
      TREE,
      WORKDIR,
      add,
      abortMerge,
      addNote,
      addRemote,
      annotatedTag,
      branch,
      cherryPick,
      checkout,
      clone,
      commit,
      getConfig,
      getConfigAll,
      setConfig,
      currentBranch,
      deleteBranch: deleteBranch2,
      deleteRef,
      deleteRemote,
      deleteTag,
      expandOid: expandOid2,
      expandRef,
      fastForward,
      fetch,
      findMergeBase: findMergeBase2,
      findRoot,
      getRemoteInfo,
      getRemoteInfo2,
      hashBlob,
      indexPack,
      init: init2,
      isDescendent: isDescendent2,
      isIgnored,
      listBranches: listBranches2,
      listFiles,
      listNotes,
      listRefs,
      listRemotes,
      listServerRefs,
      listTags,
      log: log2,
      merge: merge2,
      packObjects,
      pull,
      push,
      readBlob: readBlob2,
      readCommit: readCommit2,
      readNote,
      readObject: readObject2,
      readTag,
      readTree: readTree2,
      remove,
      removeNote,
      renameBranch,
      resetIndex,
      updateIndex: updateIndex$1,
      resolveRef: resolveRef2,
      status,
      statusMatrix,
      tag,
      version,
      walk,
      writeBlob: writeBlob2,
      writeCommit: writeCommit2,
      writeObject,
      writeRef: writeRef2,
      writeTag,
      writeTree: writeTree2,
      stash
    };
    exports.Errors = Errors;
    exports.STAGE = STAGE;
    exports.TREE = TREE;
    exports.WORKDIR = WORKDIR;
    exports.abortMerge = abortMerge;
    exports.add = add;
    exports.addNote = addNote;
    exports.addRemote = addRemote;
    exports.annotatedTag = annotatedTag;
    exports.branch = branch;
    exports.checkout = checkout;
    exports.cherryPick = cherryPick;
    exports.clone = clone;
    exports.commit = commit;
    exports.currentBranch = currentBranch;
    exports.default = index;
    exports.deleteBranch = deleteBranch2;
    exports.deleteRef = deleteRef;
    exports.deleteRemote = deleteRemote;
    exports.deleteTag = deleteTag;
    exports.expandOid = expandOid2;
    exports.expandRef = expandRef;
    exports.fastForward = fastForward;
    exports.fetch = fetch;
    exports.findMergeBase = findMergeBase2;
    exports.findRoot = findRoot;
    exports.getConfig = getConfig;
    exports.getConfigAll = getConfigAll;
    exports.getRemoteInfo = getRemoteInfo;
    exports.getRemoteInfo2 = getRemoteInfo2;
    exports.hashBlob = hashBlob;
    exports.indexPack = indexPack;
    exports.init = init2;
    exports.isDescendent = isDescendent2;
    exports.isIgnored = isIgnored;
    exports.listBranches = listBranches2;
    exports.listFiles = listFiles;
    exports.listNotes = listNotes;
    exports.listRefs = listRefs;
    exports.listRemotes = listRemotes;
    exports.listServerRefs = listServerRefs;
    exports.listTags = listTags;
    exports.log = log2;
    exports.merge = merge2;
    exports.packObjects = packObjects;
    exports.pull = pull;
    exports.push = push;
    exports.readBlob = readBlob2;
    exports.readCommit = readCommit2;
    exports.readNote = readNote;
    exports.readObject = readObject2;
    exports.readTag = readTag;
    exports.readTree = readTree2;
    exports.remove = remove;
    exports.removeNote = removeNote;
    exports.renameBranch = renameBranch;
    exports.resetIndex = resetIndex;
    exports.resolveRef = resolveRef2;
    exports.setConfig = setConfig;
    exports.stash = stash;
    exports.status = status;
    exports.statusMatrix = statusMatrix;
    exports.tag = tag;
    exports.updateIndex = updateIndex$1;
    exports.version = version;
    exports.walk = walk;
    exports.writeBlob = writeBlob2;
    exports.writeCommit = writeCommit2;
    exports.writeObject = writeObject;
    exports.writeRef = writeRef2;
    exports.writeTag = writeTag;
    exports.writeTree = writeTree2;
  }
});

// node_modules/picomatch/lib/constants.js
var require_constants2 = __commonJS({
  "node_modules/picomatch/lib/constants.js"(exports, module) {
    "use strict";
    var WIN_SLASH = "\\\\/";
    var WIN_NO_SLASH = `[^${WIN_SLASH}]`;
    var DEFAULT_MAX_EXTGLOB_RECURSION = 0;
    var DOT_LITERAL = "\\.";
    var PLUS_LITERAL = "\\+";
    var QMARK_LITERAL = "\\?";
    var SLASH_LITERAL = "\\/";
    var ONE_CHAR = "(?=.)";
    var QMARK = "[^/]";
    var END_ANCHOR = `(?:${SLASH_LITERAL}|$)`;
    var START_ANCHOR = `(?:^|${SLASH_LITERAL})`;
    var DOTS_SLASH = `${DOT_LITERAL}{1,2}${END_ANCHOR}`;
    var NO_DOT = `(?!${DOT_LITERAL})`;
    var NO_DOTS = `(?!${START_ANCHOR}${DOTS_SLASH})`;
    var NO_DOT_SLASH = `(?!${DOT_LITERAL}{0,1}${END_ANCHOR})`;
    var NO_DOTS_SLASH = `(?!${DOTS_SLASH})`;
    var QMARK_NO_DOT = `[^.${SLASH_LITERAL}]`;
    var STAR = `${QMARK}*?`;
    var SEP = "/";
    var POSIX_CHARS = {
      DOT_LITERAL,
      PLUS_LITERAL,
      QMARK_LITERAL,
      SLASH_LITERAL,
      ONE_CHAR,
      QMARK,
      END_ANCHOR,
      DOTS_SLASH,
      NO_DOT,
      NO_DOTS,
      NO_DOT_SLASH,
      NO_DOTS_SLASH,
      QMARK_NO_DOT,
      STAR,
      START_ANCHOR,
      SEP
    };
    var WINDOWS_CHARS = {
      ...POSIX_CHARS,
      SLASH_LITERAL: `[${WIN_SLASH}]`,
      QMARK: WIN_NO_SLASH,
      STAR: `${WIN_NO_SLASH}*?`,
      DOTS_SLASH: `${DOT_LITERAL}{1,2}(?:[${WIN_SLASH}]|$)`,
      NO_DOT: `(?!${DOT_LITERAL})`,
      NO_DOTS: `(?!(?:^|[${WIN_SLASH}])${DOT_LITERAL}{1,2}(?:[${WIN_SLASH}]|$))`,
      NO_DOT_SLASH: `(?!${DOT_LITERAL}{0,1}(?:[${WIN_SLASH}]|$))`,
      NO_DOTS_SLASH: `(?!${DOT_LITERAL}{1,2}(?:[${WIN_SLASH}]|$))`,
      QMARK_NO_DOT: `[^.${WIN_SLASH}]`,
      START_ANCHOR: `(?:^|[${WIN_SLASH}])`,
      END_ANCHOR: `(?:[${WIN_SLASH}]|$)`,
      SEP: "\\"
    };
    var POSIX_REGEX_SOURCE = {
      __proto__: null,
      alnum: "a-zA-Z0-9",
      alpha: "a-zA-Z",
      ascii: "\\x00-\\x7F",
      blank: " \\t",
      cntrl: "\\x00-\\x1F\\x7F",
      digit: "0-9",
      graph: "\\x21-\\x7E",
      lower: "a-z",
      print: "\\x20-\\x7E ",
      punct: "\\-!\"#$%&'()\\*+,./:;<=>?@[\\]^_`{|}~",
      space: " \\t\\r\\n\\v\\f",
      upper: "A-Z",
      word: "A-Za-z0-9_",
      xdigit: "A-Fa-f0-9"
    };
    module.exports = {
      DEFAULT_MAX_EXTGLOB_RECURSION,
      MAX_LENGTH: 1024 * 64,
      POSIX_REGEX_SOURCE,
      // regular expressions
      REGEX_BACKSLASH: /\\(?![*+?^${}(|)[\]])/g,
      REGEX_NON_SPECIAL_CHARS: /^[^@![\].,$*+?^{}()|\\/]+/,
      REGEX_SPECIAL_CHARS: /[-*+?.^${}(|)[\]]/,
      REGEX_SPECIAL_CHARS_BACKREF: /(\\?)((\W)(\3*))/g,
      REGEX_SPECIAL_CHARS_GLOBAL: /([-*+?.^${}(|)[\]])/g,
      REGEX_REMOVE_BACKSLASH: /(?:\[.*?[^\\]\]|\\(?=.))/g,
      // Replace globs with equivalent patterns to reduce parsing time.
      REPLACEMENTS: {
        __proto__: null,
        "***": "*",
        "**/**": "**",
        "**/**/**": "**"
      },
      // Digits
      CHAR_0: 48,
      /* 0 */
      CHAR_9: 57,
      /* 9 */
      // Alphabet chars.
      CHAR_UPPERCASE_A: 65,
      /* A */
      CHAR_LOWERCASE_A: 97,
      /* a */
      CHAR_UPPERCASE_Z: 90,
      /* Z */
      CHAR_LOWERCASE_Z: 122,
      /* z */
      CHAR_LEFT_PARENTHESES: 40,
      /* ( */
      CHAR_RIGHT_PARENTHESES: 41,
      /* ) */
      CHAR_ASTERISK: 42,
      /* * */
      // Non-alphabetic chars.
      CHAR_AMPERSAND: 38,
      /* & */
      CHAR_AT: 64,
      /* @ */
      CHAR_BACKWARD_SLASH: 92,
      /* \ */
      CHAR_CARRIAGE_RETURN: 13,
      /* \r */
      CHAR_CIRCUMFLEX_ACCENT: 94,
      /* ^ */
      CHAR_COLON: 58,
      /* : */
      CHAR_COMMA: 44,
      /* , */
      CHAR_DOT: 46,
      /* . */
      CHAR_DOUBLE_QUOTE: 34,
      /* " */
      CHAR_EQUAL: 61,
      /* = */
      CHAR_EXCLAMATION_MARK: 33,
      /* ! */
      CHAR_FORM_FEED: 12,
      /* \f */
      CHAR_FORWARD_SLASH: 47,
      /* / */
      CHAR_GRAVE_ACCENT: 96,
      /* ` */
      CHAR_HASH: 35,
      /* # */
      CHAR_HYPHEN_MINUS: 45,
      /* - */
      CHAR_LEFT_ANGLE_BRACKET: 60,
      /* < */
      CHAR_LEFT_CURLY_BRACE: 123,
      /* { */
      CHAR_LEFT_SQUARE_BRACKET: 91,
      /* [ */
      CHAR_LINE_FEED: 10,
      /* \n */
      CHAR_NO_BREAK_SPACE: 160,
      /* \u00A0 */
      CHAR_PERCENT: 37,
      /* % */
      CHAR_PLUS: 43,
      /* + */
      CHAR_QUESTION_MARK: 63,
      /* ? */
      CHAR_RIGHT_ANGLE_BRACKET: 62,
      /* > */
      CHAR_RIGHT_CURLY_BRACE: 125,
      /* } */
      CHAR_RIGHT_SQUARE_BRACKET: 93,
      /* ] */
      CHAR_SEMICOLON: 59,
      /* ; */
      CHAR_SINGLE_QUOTE: 39,
      /* ' */
      CHAR_SPACE: 32,
      /*   */
      CHAR_TAB: 9,
      /* \t */
      CHAR_UNDERSCORE: 95,
      /* _ */
      CHAR_VERTICAL_LINE: 124,
      /* | */
      CHAR_ZERO_WIDTH_NOBREAK_SPACE: 65279,
      /* \uFEFF */
      /**
       * Create EXTGLOB_CHARS
       */
      extglobChars(chars) {
        return {
          "!": { type: "negate", open: "(?:(?!(?:", close: `))${chars.STAR})` },
          "?": { type: "qmark", open: "(?:", close: ")?" },
          "+": { type: "plus", open: "(?:", close: ")+" },
          "*": { type: "star", open: "(?:", close: ")*" },
          "@": { type: "at", open: "(?:", close: ")" }
        };
      },
      /**
       * Create GLOB_CHARS
       */
      globChars(win32) {
        return win32 === true ? WINDOWS_CHARS : POSIX_CHARS;
      }
    };
  }
});

// node_modules/picomatch/lib/utils.js
var require_utils = __commonJS({
  "node_modules/picomatch/lib/utils.js"(exports) {
    "use strict";
    var {
      REGEX_BACKSLASH,
      REGEX_REMOVE_BACKSLASH,
      REGEX_SPECIAL_CHARS,
      REGEX_SPECIAL_CHARS_GLOBAL
    } = require_constants2();
    exports.isObject = (val) => val !== null && typeof val === "object" && !Array.isArray(val);
    exports.hasRegexChars = (str) => REGEX_SPECIAL_CHARS.test(str);
    exports.isRegexChar = (str) => str.length === 1 && exports.hasRegexChars(str);
    exports.escapeRegex = (str) => str.replace(REGEX_SPECIAL_CHARS_GLOBAL, "\\$1");
    exports.toPosixSlashes = (str) => str.replace(REGEX_BACKSLASH, "/");
    exports.isWindows = () => {
      if (typeof navigator !== "undefined" && navigator.platform) {
        const platform = navigator.platform.toLowerCase();
        return platform === "win32" || platform === "windows";
      }
      if (typeof process !== "undefined" && process.platform) {
        return process.platform === "win32";
      }
      return false;
    };
    exports.removeBackslashes = (str) => {
      return str.replace(REGEX_REMOVE_BACKSLASH, (match) => {
        return match === "\\" ? "" : match;
      });
    };
    exports.escapeLast = (input2, char, lastIdx) => {
      const idx = input2.lastIndexOf(char, lastIdx);
      if (idx === -1) return input2;
      if (input2[idx - 1] === "\\") return exports.escapeLast(input2, char, idx - 1);
      return `${input2.slice(0, idx)}\\${input2.slice(idx)}`;
    };
    exports.removePrefix = (input2, state = {}) => {
      let output2 = input2;
      if (output2.startsWith("./")) {
        output2 = output2.slice(2);
        state.prefix = "./";
      }
      return output2;
    };
    exports.wrapOutput = (input2, state = {}, options = {}) => {
      const prepend = options.contains ? "" : "^";
      const append = options.contains ? "" : "$";
      let output2 = `${prepend}(?:${input2})${append}`;
      if (state.negated === true) {
        output2 = `(?:^(?!${output2}).*$)`;
      }
      return output2;
    };
    exports.basename = (path3, { windows } = {}) => {
      const segs = path3.split(windows ? /[\\/]/ : "/");
      const last = segs[segs.length - 1];
      if (last === "") {
        return segs[segs.length - 2];
      }
      return last;
    };
  }
});

// node_modules/picomatch/lib/scan.js
var require_scan = __commonJS({
  "node_modules/picomatch/lib/scan.js"(exports, module) {
    "use strict";
    var utils = require_utils();
    var {
      CHAR_ASTERISK,
      /* * */
      CHAR_AT,
      /* @ */
      CHAR_BACKWARD_SLASH,
      /* \ */
      CHAR_COMMA,
      /* , */
      CHAR_DOT,
      /* . */
      CHAR_EXCLAMATION_MARK,
      /* ! */
      CHAR_FORWARD_SLASH,
      /* / */
      CHAR_LEFT_CURLY_BRACE,
      /* { */
      CHAR_LEFT_PARENTHESES,
      /* ( */
      CHAR_LEFT_SQUARE_BRACKET,
      /* [ */
      CHAR_PLUS,
      /* + */
      CHAR_QUESTION_MARK,
      /* ? */
      CHAR_RIGHT_CURLY_BRACE,
      /* } */
      CHAR_RIGHT_PARENTHESES,
      /* ) */
      CHAR_RIGHT_SQUARE_BRACKET
      /* ] */
    } = require_constants2();
    var isPathSeparator = (code) => {
      return code === CHAR_FORWARD_SLASH || code === CHAR_BACKWARD_SLASH;
    };
    var depth = (token) => {
      if (token.isPrefix !== true) {
        token.depth = token.isGlobstar ? Infinity : 1;
      }
    };
    var scan = (input2, options) => {
      const opts = options || {};
      const length = input2.length - 1;
      const scanToEnd = opts.parts === true || opts.scanToEnd === true;
      const slashes = [];
      const tokens = [];
      const parts = [];
      let str = input2;
      let index = -1;
      let start = 0;
      let lastIndex = 0;
      let isBrace = false;
      let isBracket = false;
      let isGlob = false;
      let isExtglob = false;
      let isGlobstar = false;
      let braceEscaped = false;
      let backslashes = false;
      let negated = false;
      let negatedExtglob = false;
      let finished = false;
      let braces = 0;
      let prev;
      let code;
      let token = { value: "", depth: 0, isGlob: false };
      const eos = () => index >= length;
      const peek = () => str.charCodeAt(index + 1);
      const advance = () => {
        prev = code;
        return str.charCodeAt(++index);
      };
      while (index < length) {
        code = advance();
        let next;
        if (code === CHAR_BACKWARD_SLASH) {
          backslashes = token.backslashes = true;
          code = advance();
          if (code === CHAR_LEFT_CURLY_BRACE) {
            braceEscaped = true;
          }
          continue;
        }
        if (braceEscaped === true || code === CHAR_LEFT_CURLY_BRACE) {
          braces++;
          while (eos() !== true && (code = advance())) {
            if (code === CHAR_BACKWARD_SLASH) {
              backslashes = token.backslashes = true;
              advance();
              continue;
            }
            if (code === CHAR_LEFT_CURLY_BRACE) {
              braces++;
              continue;
            }
            if (braceEscaped !== true && code === CHAR_DOT && (code = advance()) === CHAR_DOT) {
              isBrace = token.isBrace = true;
              isGlob = token.isGlob = true;
              finished = true;
              if (scanToEnd === true) {
                continue;
              }
              break;
            }
            if (braceEscaped !== true && code === CHAR_COMMA) {
              isBrace = token.isBrace = true;
              isGlob = token.isGlob = true;
              finished = true;
              if (scanToEnd === true) {
                continue;
              }
              break;
            }
            if (code === CHAR_RIGHT_CURLY_BRACE) {
              braces--;
              if (braces === 0) {
                braceEscaped = false;
                isBrace = token.isBrace = true;
                finished = true;
                break;
              }
            }
          }
          if (scanToEnd === true) {
            continue;
          }
          break;
        }
        if (code === CHAR_FORWARD_SLASH) {
          slashes.push(index);
          tokens.push(token);
          token = { value: "", depth: 0, isGlob: false };
          if (finished === true) continue;
          if (prev === CHAR_DOT && index === start + 1) {
            start += 2;
            continue;
          }
          lastIndex = index + 1;
          continue;
        }
        if (opts.noext !== true) {
          const isExtglobChar = code === CHAR_PLUS || code === CHAR_AT || code === CHAR_ASTERISK || code === CHAR_QUESTION_MARK || code === CHAR_EXCLAMATION_MARK;
          if (isExtglobChar === true && peek() === CHAR_LEFT_PARENTHESES) {
            isGlob = token.isGlob = true;
            isExtglob = token.isExtglob = true;
            finished = true;
            if (code === CHAR_EXCLAMATION_MARK && index === start) {
              negatedExtglob = true;
            }
            if (scanToEnd === true) {
              while (eos() !== true && (code = advance())) {
                if (code === CHAR_BACKWARD_SLASH) {
                  backslashes = token.backslashes = true;
                  code = advance();
                  continue;
                }
                if (code === CHAR_RIGHT_PARENTHESES) {
                  isGlob = token.isGlob = true;
                  finished = true;
                  break;
                }
              }
              continue;
            }
            break;
          }
        }
        if (code === CHAR_ASTERISK) {
          if (prev === CHAR_ASTERISK) isGlobstar = token.isGlobstar = true;
          isGlob = token.isGlob = true;
          finished = true;
          if (scanToEnd === true) {
            continue;
          }
          break;
        }
        if (code === CHAR_QUESTION_MARK) {
          isGlob = token.isGlob = true;
          finished = true;
          if (scanToEnd === true) {
            continue;
          }
          break;
        }
        if (code === CHAR_LEFT_SQUARE_BRACKET) {
          while (eos() !== true && (next = advance())) {
            if (next === CHAR_BACKWARD_SLASH) {
              backslashes = token.backslashes = true;
              advance();
              continue;
            }
            if (next === CHAR_RIGHT_SQUARE_BRACKET) {
              isBracket = token.isBracket = true;
              isGlob = token.isGlob = true;
              finished = true;
              break;
            }
          }
          if (scanToEnd === true) {
            continue;
          }
          break;
        }
        if (opts.nonegate !== true && code === CHAR_EXCLAMATION_MARK && index === start) {
          negated = token.negated = true;
          start++;
          continue;
        }
        if (opts.noparen !== true && code === CHAR_LEFT_PARENTHESES) {
          isGlob = token.isGlob = true;
          if (scanToEnd === true) {
            while (eos() !== true && (code = advance())) {
              if (code === CHAR_LEFT_PARENTHESES) {
                backslashes = token.backslashes = true;
                code = advance();
                continue;
              }
              if (code === CHAR_RIGHT_PARENTHESES) {
                finished = true;
                break;
              }
            }
            continue;
          }
          break;
        }
        if (isGlob === true) {
          finished = true;
          if (scanToEnd === true) {
            continue;
          }
          break;
        }
      }
      if (opts.noext === true) {
        isExtglob = false;
        isGlob = false;
      }
      let base = str;
      let prefix = "";
      let glob = "";
      if (start > 0) {
        prefix = str.slice(0, start);
        str = str.slice(start);
        lastIndex -= start;
      }
      if (base && isGlob === true && lastIndex > 0) {
        base = str.slice(0, lastIndex);
        glob = str.slice(lastIndex);
      } else if (isGlob === true) {
        base = "";
        glob = str;
      } else {
        base = str;
      }
      if (base && base !== "" && base !== "/" && base !== str) {
        if (isPathSeparator(base.charCodeAt(base.length - 1))) {
          base = base.slice(0, -1);
        }
      }
      if (opts.unescape === true) {
        if (glob) glob = utils.removeBackslashes(glob);
        if (base && backslashes === true) {
          base = utils.removeBackslashes(base);
        }
      }
      const state = {
        prefix,
        input: input2,
        start,
        base,
        glob,
        isBrace,
        isBracket,
        isGlob,
        isExtglob,
        isGlobstar,
        negated,
        negatedExtglob
      };
      if (opts.tokens === true) {
        state.maxDepth = 0;
        if (!isPathSeparator(code)) {
          tokens.push(token);
        }
        state.tokens = tokens;
      }
      if (opts.parts === true || opts.tokens === true) {
        let prevIndex;
        for (let idx = 0; idx < slashes.length; idx++) {
          const n = prevIndex ? prevIndex + 1 : start;
          const i = slashes[idx];
          const value = input2.slice(n, i);
          if (opts.tokens) {
            if (idx === 0 && start !== 0) {
              tokens[idx].isPrefix = true;
              tokens[idx].value = prefix;
            } else {
              tokens[idx].value = value;
            }
            depth(tokens[idx]);
            state.maxDepth += tokens[idx].depth;
          }
          if (idx !== 0 || value !== "") {
            parts.push(value);
          }
          prevIndex = i;
        }
        if (prevIndex && prevIndex + 1 < input2.length) {
          const value = input2.slice(prevIndex + 1);
          parts.push(value);
          if (opts.tokens) {
            tokens[tokens.length - 1].value = value;
            depth(tokens[tokens.length - 1]);
            state.maxDepth += tokens[tokens.length - 1].depth;
          }
        }
        state.slashes = slashes;
        state.parts = parts;
      }
      return state;
    };
    module.exports = scan;
  }
});

// node_modules/picomatch/lib/parse.js
var require_parse = __commonJS({
  "node_modules/picomatch/lib/parse.js"(exports, module) {
    "use strict";
    var constants = require_constants2();
    var utils = require_utils();
    var {
      MAX_LENGTH,
      POSIX_REGEX_SOURCE,
      REGEX_NON_SPECIAL_CHARS,
      REGEX_SPECIAL_CHARS_BACKREF,
      REPLACEMENTS
    } = constants;
    var expandRange = (args, options) => {
      if (typeof options.expandRange === "function") {
        return options.expandRange(...args, options);
      }
      args.sort();
      const value = `[${args.join("-")}]`;
      try {
        new RegExp(value);
      } catch (ex) {
        return args.map((v) => utils.escapeRegex(v)).join("..");
      }
      return value;
    };
    var syntaxError = (type, char) => {
      return `Missing ${type}: "${char}" - use "\\\\${char}" to match literal characters`;
    };
    var splitTopLevel = (input2) => {
      const parts = [];
      let bracket = 0;
      let paren = 0;
      let quote = 0;
      let value = "";
      let escaped = false;
      for (const ch of input2) {
        if (escaped === true) {
          value += ch;
          escaped = false;
          continue;
        }
        if (ch === "\\") {
          value += ch;
          escaped = true;
          continue;
        }
        if (ch === '"') {
          quote = quote === 1 ? 0 : 1;
          value += ch;
          continue;
        }
        if (quote === 0) {
          if (ch === "[") {
            bracket++;
          } else if (ch === "]" && bracket > 0) {
            bracket--;
          } else if (bracket === 0) {
            if (ch === "(") {
              paren++;
            } else if (ch === ")" && paren > 0) {
              paren--;
            } else if (ch === "|" && paren === 0) {
              parts.push(value);
              value = "";
              continue;
            }
          }
        }
        value += ch;
      }
      parts.push(value);
      return parts;
    };
    var isPlainBranch = (branch) => {
      let escaped = false;
      for (const ch of branch) {
        if (escaped === true) {
          escaped = false;
          continue;
        }
        if (ch === "\\") {
          escaped = true;
          continue;
        }
        if (/[?*+@!()[\]{}]/.test(ch)) {
          return false;
        }
      }
      return true;
    };
    var normalizeSimpleBranch = (branch) => {
      let value = branch.trim();
      let changed = true;
      while (changed === true) {
        changed = false;
        if (/^@\([^\\()[\]{}|]+\)$/.test(value)) {
          value = value.slice(2, -1);
          changed = true;
        }
      }
      if (!isPlainBranch(value)) {
        return;
      }
      return value.replace(/\\(.)/g, "$1");
    };
    var hasRepeatedCharPrefixOverlap = (branches) => {
      const values = branches.map(normalizeSimpleBranch).filter(Boolean);
      for (let i = 0; i < values.length; i++) {
        for (let j = i + 1; j < values.length; j++) {
          const a = values[i];
          const b = values[j];
          const char = a[0];
          if (!char || a !== char.repeat(a.length) || b !== char.repeat(b.length)) {
            continue;
          }
          if (a === b || a.startsWith(b) || b.startsWith(a)) {
            return true;
          }
        }
      }
      return false;
    };
    var parseRepeatedExtglob = (pattern, requireEnd = true) => {
      if (pattern[0] !== "+" && pattern[0] !== "*" || pattern[1] !== "(") {
        return;
      }
      let bracket = 0;
      let paren = 0;
      let quote = 0;
      let escaped = false;
      for (let i = 1; i < pattern.length; i++) {
        const ch = pattern[i];
        if (escaped === true) {
          escaped = false;
          continue;
        }
        if (ch === "\\") {
          escaped = true;
          continue;
        }
        if (ch === '"') {
          quote = quote === 1 ? 0 : 1;
          continue;
        }
        if (quote === 1) {
          continue;
        }
        if (ch === "[") {
          bracket++;
          continue;
        }
        if (ch === "]" && bracket > 0) {
          bracket--;
          continue;
        }
        if (bracket > 0) {
          continue;
        }
        if (ch === "(") {
          paren++;
          continue;
        }
        if (ch === ")") {
          paren--;
          if (paren === 0) {
            if (requireEnd === true && i !== pattern.length - 1) {
              return;
            }
            return {
              type: pattern[0],
              body: pattern.slice(2, i),
              end: i
            };
          }
        }
      }
    };
    var getStarExtglobSequenceOutput = (pattern) => {
      let index = 0;
      const chars = [];
      while (index < pattern.length) {
        const match = parseRepeatedExtglob(pattern.slice(index), false);
        if (!match || match.type !== "*") {
          return;
        }
        const branches = splitTopLevel(match.body).map((branch2) => branch2.trim());
        if (branches.length !== 1) {
          return;
        }
        const branch = normalizeSimpleBranch(branches[0]);
        if (!branch || branch.length !== 1) {
          return;
        }
        chars.push(branch);
        index += match.end + 1;
      }
      if (chars.length < 1) {
        return;
      }
      const source = chars.length === 1 ? utils.escapeRegex(chars[0]) : `[${chars.map((ch) => utils.escapeRegex(ch)).join("")}]`;
      return `${source}*`;
    };
    var repeatedExtglobRecursion = (pattern) => {
      let depth = 0;
      let value = pattern.trim();
      let match = parseRepeatedExtglob(value);
      while (match) {
        depth++;
        value = match.body.trim();
        match = parseRepeatedExtglob(value);
      }
      return depth;
    };
    var analyzeRepeatedExtglob = (body, options) => {
      if (options.maxExtglobRecursion === false) {
        return { risky: false };
      }
      const max = typeof options.maxExtglobRecursion === "number" ? options.maxExtglobRecursion : constants.DEFAULT_MAX_EXTGLOB_RECURSION;
      const branches = splitTopLevel(body).map((branch) => branch.trim());
      if (branches.length > 1) {
        if (branches.some((branch) => branch === "") || branches.some((branch) => /^[*?]+$/.test(branch)) || hasRepeatedCharPrefixOverlap(branches)) {
          return { risky: true };
        }
      }
      for (const branch of branches) {
        const safeOutput = getStarExtglobSequenceOutput(branch);
        if (safeOutput) {
          return { risky: true, safeOutput };
        }
        if (repeatedExtglobRecursion(branch) > max) {
          return { risky: true };
        }
      }
      return { risky: false };
    };
    var parse = (input2, options) => {
      if (typeof input2 !== "string") {
        throw new TypeError("Expected a string");
      }
      input2 = REPLACEMENTS[input2] || input2;
      const opts = { ...options };
      const max = typeof opts.maxLength === "number" ? Math.min(MAX_LENGTH, opts.maxLength) : MAX_LENGTH;
      let len = input2.length;
      if (len > max) {
        throw new SyntaxError(`Input length: ${len}, exceeds maximum allowed length: ${max}`);
      }
      const bos = { type: "bos", value: "", output: opts.prepend || "" };
      const tokens = [bos];
      const capture = opts.capture ? "" : "?:";
      const PLATFORM_CHARS = constants.globChars(opts.windows);
      const EXTGLOB_CHARS = constants.extglobChars(PLATFORM_CHARS);
      const {
        DOT_LITERAL,
        PLUS_LITERAL,
        SLASH_LITERAL,
        ONE_CHAR,
        DOTS_SLASH,
        NO_DOT,
        NO_DOT_SLASH,
        NO_DOTS_SLASH,
        QMARK,
        QMARK_NO_DOT,
        STAR,
        START_ANCHOR
      } = PLATFORM_CHARS;
      const globstar = (opts2) => {
        return `(${capture}(?:(?!${START_ANCHOR}${opts2.dot ? DOTS_SLASH : DOT_LITERAL}).)*?)`;
      };
      const nodot = opts.dot ? "" : NO_DOT;
      const qmarkNoDot = opts.dot ? QMARK : QMARK_NO_DOT;
      let star = opts.bash === true ? globstar(opts) : STAR;
      if (opts.capture) {
        star = `(${star})`;
      }
      if (typeof opts.noext === "boolean") {
        opts.noextglob = opts.noext;
      }
      const state = {
        input: input2,
        index: -1,
        start: 0,
        dot: opts.dot === true,
        consumed: "",
        output: "",
        prefix: "",
        backtrack: false,
        negated: false,
        brackets: 0,
        braces: 0,
        parens: 0,
        quotes: 0,
        globstar: false,
        tokens
      };
      input2 = utils.removePrefix(input2, state);
      len = input2.length;
      const extglobs = [];
      const braces = [];
      const stack = [];
      let prev = bos;
      let value;
      const eos = () => state.index === len - 1;
      const peek = state.peek = (n = 1) => input2[state.index + n];
      const advance = state.advance = () => input2[++state.index] || "";
      const remaining = () => input2.slice(state.index + 1);
      const consume = (value2 = "", num = 0) => {
        state.consumed += value2;
        state.index += num;
      };
      const append = (token) => {
        state.output += token.output != null ? token.output : token.value;
        consume(token.value);
      };
      const negate = () => {
        let count = 1;
        while (peek() === "!" && (peek(2) !== "(" || peek(3) === "?")) {
          advance();
          state.start++;
          count++;
        }
        if (count % 2 === 0) {
          return false;
        }
        state.negated = true;
        state.start++;
        return true;
      };
      const increment = (type) => {
        state[type]++;
        stack.push(type);
      };
      const decrement = (type) => {
        state[type]--;
        stack.pop();
      };
      const push = (tok) => {
        if (prev.type === "globstar") {
          const isBrace = state.braces > 0 && (tok.type === "comma" || tok.type === "brace");
          const isExtglob = tok.extglob === true || extglobs.length && (tok.type === "pipe" || tok.type === "paren");
          if (tok.type !== "slash" && tok.type !== "paren" && !isBrace && !isExtglob) {
            state.output = state.output.slice(0, -prev.output.length);
            prev.type = "star";
            prev.value = "*";
            prev.output = star;
            state.output += prev.output;
          }
        }
        if (extglobs.length && tok.type !== "paren") {
          extglobs[extglobs.length - 1].inner += tok.value;
        }
        if (tok.value || tok.output) append(tok);
        if (prev && prev.type === "text" && tok.type === "text") {
          prev.output = (prev.output || prev.value) + tok.value;
          prev.value += tok.value;
          return;
        }
        tok.prev = prev;
        tokens.push(tok);
        prev = tok;
      };
      const extglobOpen = (type, value2) => {
        const token = { ...EXTGLOB_CHARS[value2], conditions: 1, inner: "" };
        token.prev = prev;
        token.parens = state.parens;
        token.output = state.output;
        token.startIndex = state.index;
        token.tokensIndex = tokens.length;
        const output2 = (opts.capture ? "(" : "") + token.open;
        increment("parens");
        push({ type, value: value2, output: state.output ? "" : ONE_CHAR });
        push({ type: "paren", extglob: true, value: advance(), output: output2 });
        extglobs.push(token);
      };
      const extglobClose = (token) => {
        const literal = input2.slice(token.startIndex, state.index + 1);
        const body = input2.slice(token.startIndex + 2, state.index);
        const analysis = analyzeRepeatedExtglob(body, opts);
        if ((token.type === "plus" || token.type === "star") && analysis.risky) {
          const safeOutput = analysis.safeOutput ? (token.output ? "" : ONE_CHAR) + (opts.capture ? `(${analysis.safeOutput})` : analysis.safeOutput) : void 0;
          const open = tokens[token.tokensIndex];
          open.type = "text";
          open.value = literal;
          open.output = safeOutput || utils.escapeRegex(literal);
          for (let i = token.tokensIndex + 1; i < tokens.length; i++) {
            tokens[i].value = "";
            tokens[i].output = "";
            delete tokens[i].suffix;
          }
          state.output = token.output + open.output;
          state.backtrack = true;
          push({ type: "paren", extglob: true, value, output: "" });
          decrement("parens");
          return;
        }
        let output2 = token.close + (opts.capture ? ")" : "");
        let rest;
        if (token.type === "negate") {
          let extglobStar = star;
          if (token.inner && token.inner.length > 1 && token.inner.includes("/")) {
            extglobStar = globstar(opts);
          }
          if (extglobStar !== star || eos() || /^\)+$/.test(remaining())) {
            output2 = token.close = `)$))${extglobStar}`;
          }
          if (token.inner.includes("*") && (rest = remaining()) && /^\.[^\\/.]+$/.test(rest)) {
            const expression = parse(rest, { ...options, fastpaths: false }).output;
            output2 = token.close = `)${expression})${extglobStar})`;
          }
          if (token.prev.type === "bos") {
            state.negatedExtglob = true;
          }
        }
        push({ type: "paren", extglob: true, value, output: output2 });
        decrement("parens");
      };
      if (opts.fastpaths !== false && !/(^[*!]|[/()[\]{}"])/.test(input2)) {
        let backslashes = false;
        let output2 = input2.replace(REGEX_SPECIAL_CHARS_BACKREF, (m, esc, chars, first, rest, index) => {
          if (first === "\\") {
            backslashes = true;
            return m;
          }
          if (first === "?") {
            if (esc) {
              return esc + first + (rest ? QMARK.repeat(rest.length) : "");
            }
            if (index === 0) {
              return qmarkNoDot + (rest ? QMARK.repeat(rest.length) : "");
            }
            return QMARK.repeat(chars.length);
          }
          if (first === ".") {
            return DOT_LITERAL.repeat(chars.length);
          }
          if (first === "*") {
            if (esc) {
              return esc + first + (rest ? star : "");
            }
            return star;
          }
          return esc ? m : `\\${m}`;
        });
        if (backslashes === true) {
          if (opts.unescape === true) {
            output2 = output2.replace(/\\/g, "");
          } else {
            output2 = output2.replace(/\\+/g, (m) => {
              return m.length % 2 === 0 ? "\\\\" : m ? "\\" : "";
            });
          }
        }
        if (output2 === input2 && opts.contains === true) {
          state.output = input2;
          return state;
        }
        state.output = utils.wrapOutput(output2, state, options);
        return state;
      }
      while (!eos()) {
        value = advance();
        if (value === "\0") {
          continue;
        }
        if (value === "\\") {
          const next = peek();
          if (next === "/" && opts.bash !== true) {
            continue;
          }
          if (next === "." || next === ";") {
            continue;
          }
          if (!next) {
            value += "\\";
            push({ type: "text", value });
            continue;
          }
          const match = /^\\+/.exec(remaining());
          let slashes = 0;
          if (match && match[0].length > 2) {
            slashes = match[0].length;
            state.index += slashes;
            if (slashes % 2 !== 0) {
              value += "\\";
            }
          }
          if (opts.unescape === true) {
            value = advance();
          } else {
            value += advance();
          }
          if (state.brackets === 0) {
            push({ type: "text", value });
            continue;
          }
        }
        if (state.brackets > 0 && (value !== "]" || prev.value === "[" || prev.value === "[^")) {
          if (opts.posix !== false && value === ":") {
            const inner = prev.value.slice(1);
            if (inner.includes("[")) {
              prev.posix = true;
              if (inner.includes(":")) {
                const idx = prev.value.lastIndexOf("[");
                const pre = prev.value.slice(0, idx);
                const rest2 = prev.value.slice(idx + 2);
                const posix = POSIX_REGEX_SOURCE[rest2];
                if (posix) {
                  prev.value = pre + posix;
                  state.backtrack = true;
                  advance();
                  if (!bos.output && tokens.indexOf(prev) === 1) {
                    bos.output = ONE_CHAR;
                  }
                  continue;
                }
              }
            }
          }
          if (value === "[" && peek() !== ":" || value === "-" && peek() === "]") {
            value = `\\${value}`;
          }
          if (value === "]" && (prev.value === "[" || prev.value === "[^")) {
            value = `\\${value}`;
          }
          if (opts.posix === true && value === "!" && prev.value === "[") {
            value = "^";
          }
          prev.value += value;
          append({ value });
          continue;
        }
        if (state.quotes === 1 && value !== '"') {
          value = utils.escapeRegex(value);
          prev.value += value;
          append({ value });
          continue;
        }
        if (value === '"') {
          state.quotes = state.quotes === 1 ? 0 : 1;
          if (opts.keepQuotes === true) {
            push({ type: "text", value });
          }
          continue;
        }
        if (value === "(") {
          increment("parens");
          push({ type: "paren", value });
          continue;
        }
        if (value === ")") {
          if (state.parens === 0 && opts.strictBrackets === true) {
            throw new SyntaxError(syntaxError("opening", "("));
          }
          const extglob = extglobs[extglobs.length - 1];
          if (extglob && state.parens === extglob.parens + 1) {
            extglobClose(extglobs.pop());
            continue;
          }
          push({ type: "paren", value, output: state.parens ? ")" : "\\)" });
          decrement("parens");
          continue;
        }
        if (value === "[") {
          if (opts.nobracket === true || !remaining().includes("]")) {
            if (opts.nobracket !== true && opts.strictBrackets === true) {
              throw new SyntaxError(syntaxError("closing", "]"));
            }
            value = `\\${value}`;
          } else {
            increment("brackets");
          }
          push({ type: "bracket", value });
          continue;
        }
        if (value === "]") {
          if (opts.nobracket === true || prev && prev.type === "bracket" && prev.value.length === 1) {
            push({ type: "text", value, output: `\\${value}` });
            continue;
          }
          if (state.brackets === 0) {
            if (opts.strictBrackets === true) {
              throw new SyntaxError(syntaxError("opening", "["));
            }
            push({ type: "text", value, output: `\\${value}` });
            continue;
          }
          decrement("brackets");
          const prevValue = prev.value.slice(1);
          if (prev.posix !== true && prevValue[0] === "^" && !prevValue.includes("/")) {
            value = `/${value}`;
          }
          prev.value += value;
          append({ value });
          if (opts.literalBrackets === false || utils.hasRegexChars(prevValue)) {
            continue;
          }
          const escaped = utils.escapeRegex(prev.value);
          state.output = state.output.slice(0, -prev.value.length);
          if (opts.literalBrackets === true) {
            state.output += escaped;
            prev.value = escaped;
            continue;
          }
          prev.value = `(${capture}${escaped}|${prev.value})`;
          state.output += prev.value;
          continue;
        }
        if (value === "{" && opts.nobrace !== true) {
          increment("braces");
          const open = {
            type: "brace",
            value,
            output: "(",
            outputIndex: state.output.length,
            tokensIndex: state.tokens.length
          };
          braces.push(open);
          push(open);
          continue;
        }
        if (value === "}") {
          const brace = braces[braces.length - 1];
          if (opts.nobrace === true || !brace) {
            push({ type: "text", value, output: value });
            continue;
          }
          let output2 = ")";
          if (brace.dots === true) {
            const arr = tokens.slice();
            const range = [];
            for (let i = arr.length - 1; i >= 0; i--) {
              tokens.pop();
              if (arr[i].type === "brace") {
                break;
              }
              if (arr[i].type !== "dots") {
                range.unshift(arr[i].value);
              }
            }
            output2 = expandRange(range, opts);
            state.backtrack = true;
          }
          if (brace.comma !== true && brace.dots !== true) {
            const out = state.output.slice(0, brace.outputIndex);
            const toks = state.tokens.slice(brace.tokensIndex);
            brace.value = brace.output = "\\{";
            value = output2 = "\\}";
            state.output = out;
            for (const t of toks) {
              state.output += t.output || t.value;
            }
          }
          push({ type: "brace", value, output: output2 });
          decrement("braces");
          braces.pop();
          continue;
        }
        if (value === "|") {
          if (extglobs.length > 0) {
            extglobs[extglobs.length - 1].conditions++;
          }
          push({ type: "text", value });
          continue;
        }
        if (value === ",") {
          let output2 = value;
          const brace = braces[braces.length - 1];
          if (brace && stack[stack.length - 1] === "braces") {
            brace.comma = true;
            output2 = "|";
          }
          push({ type: "comma", value, output: output2 });
          continue;
        }
        if (value === "/") {
          if (prev.type === "dot" && state.index === state.start + 1) {
            state.start = state.index + 1;
            state.consumed = "";
            state.output = "";
            tokens.pop();
            prev = bos;
            continue;
          }
          push({ type: "slash", value, output: SLASH_LITERAL });
          continue;
        }
        if (value === ".") {
          if (state.braces > 0 && prev.type === "dot") {
            if (prev.value === ".") prev.output = DOT_LITERAL;
            const brace = braces[braces.length - 1];
            prev.type = "dots";
            prev.output += value;
            prev.value += value;
            brace.dots = true;
            continue;
          }
          if (state.braces + state.parens === 0 && prev.type !== "bos" && prev.type !== "slash") {
            push({ type: "text", value, output: DOT_LITERAL });
            continue;
          }
          push({ type: "dot", value, output: DOT_LITERAL });
          continue;
        }
        if (value === "?") {
          const isGroup = prev && prev.value === "(";
          if (!isGroup && opts.noextglob !== true && peek() === "(" && peek(2) !== "?") {
            extglobOpen("qmark", value);
            continue;
          }
          if (prev && prev.type === "paren") {
            const next = peek();
            let output2 = value;
            if (prev.value === "(" && !/[!=<:]/.test(next) || next === "<" && !/<([!=]|\w+>)/.test(remaining())) {
              output2 = `\\${value}`;
            }
            push({ type: "text", value, output: output2 });
            continue;
          }
          if (opts.dot !== true && (prev.type === "slash" || prev.type === "bos")) {
            push({ type: "qmark", value, output: QMARK_NO_DOT });
            continue;
          }
          push({ type: "qmark", value, output: QMARK });
          continue;
        }
        if (value === "!") {
          if (opts.noextglob !== true && peek() === "(") {
            if (peek(2) !== "?" || !/[!=<:]/.test(peek(3))) {
              extglobOpen("negate", value);
              continue;
            }
          }
          if (opts.nonegate !== true && state.index === 0) {
            negate();
            continue;
          }
        }
        if (value === "+") {
          if (opts.noextglob !== true && peek() === "(" && peek(2) !== "?") {
            extglobOpen("plus", value);
            continue;
          }
          if (prev && prev.value === "(" || opts.regex === false) {
            push({ type: "plus", value, output: PLUS_LITERAL });
            continue;
          }
          if (prev && (prev.type === "bracket" || prev.type === "paren" || prev.type === "brace") || state.parens > 0) {
            push({ type: "plus", value });
            continue;
          }
          push({ type: "plus", value: PLUS_LITERAL });
          continue;
        }
        if (value === "@") {
          if (opts.noextglob !== true && peek() === "(" && peek(2) !== "?") {
            push({ type: "at", extglob: true, value, output: "" });
            continue;
          }
          push({ type: "text", value });
          continue;
        }
        if (value !== "*") {
          if (value === "$" || value === "^") {
            value = `\\${value}`;
          }
          const match = REGEX_NON_SPECIAL_CHARS.exec(remaining());
          if (match) {
            value += match[0];
            state.index += match[0].length;
          }
          push({ type: "text", value });
          continue;
        }
        if (prev && (prev.type === "globstar" || prev.star === true)) {
          prev.type = "star";
          prev.star = true;
          prev.value += value;
          prev.output = star;
          state.backtrack = true;
          state.globstar = true;
          consume(value);
          continue;
        }
        let rest = remaining();
        if (opts.noextglob !== true && /^\([^?]/.test(rest)) {
          extglobOpen("star", value);
          continue;
        }
        if (prev.type === "star") {
          if (opts.noglobstar === true) {
            consume(value);
            continue;
          }
          const prior = prev.prev;
          const before = prior.prev;
          const isStart = prior.type === "slash" || prior.type === "bos";
          const afterStar = before && (before.type === "star" || before.type === "globstar");
          if (opts.bash === true && (!isStart || rest[0] && rest[0] !== "/")) {
            push({ type: "star", value, output: "" });
            continue;
          }
          const isBrace = state.braces > 0 && (prior.type === "comma" || prior.type === "brace");
          const isExtglob = extglobs.length && (prior.type === "pipe" || prior.type === "paren");
          if (!isStart && prior.type !== "paren" && !isBrace && !isExtglob) {
            push({ type: "star", value, output: "" });
            continue;
          }
          while (rest.slice(0, 3) === "/**") {
            const after = input2[state.index + 4];
            if (after && after !== "/") {
              break;
            }
            rest = rest.slice(3);
            consume("/**", 3);
          }
          if (prior.type === "bos" && eos()) {
            prev.type = "globstar";
            prev.value += value;
            prev.output = globstar(opts);
            state.output = prev.output;
            state.globstar = true;
            consume(value);
            continue;
          }
          if (prior.type === "slash" && prior.prev.type !== "bos" && !afterStar && eos()) {
            state.output = state.output.slice(0, -(prior.output + prev.output).length);
            prior.output = `(?:${prior.output}`;
            prev.type = "globstar";
            prev.output = globstar(opts) + (opts.strictSlashes ? ")" : "|$)");
            prev.value += value;
            state.globstar = true;
            state.output += prior.output + prev.output;
            consume(value);
            continue;
          }
          if (prior.type === "slash" && prior.prev.type !== "bos" && rest[0] === "/") {
            const end = rest[1] !== void 0 ? "|$" : "";
            state.output = state.output.slice(0, -(prior.output + prev.output).length);
            prior.output = `(?:${prior.output}`;
            prev.type = "globstar";
            prev.output = `${globstar(opts)}${SLASH_LITERAL}|${SLASH_LITERAL}${end})`;
            prev.value += value;
            state.output += prior.output + prev.output;
            state.globstar = true;
            consume(value + advance());
            push({ type: "slash", value: "/", output: "" });
            continue;
          }
          if (prior.type === "bos" && rest[0] === "/") {
            prev.type = "globstar";
            prev.value += value;
            prev.output = `(?:^|${SLASH_LITERAL}|${globstar(opts)}${SLASH_LITERAL})`;
            state.output = prev.output;
            state.globstar = true;
            consume(value + advance());
            push({ type: "slash", value: "/", output: "" });
            continue;
          }
          state.output = state.output.slice(0, -prev.output.length);
          prev.type = "globstar";
          prev.output = globstar(opts);
          prev.value += value;
          state.output += prev.output;
          state.globstar = true;
          consume(value);
          continue;
        }
        const token = { type: "star", value, output: star };
        if (opts.bash === true) {
          token.output = ".*?";
          if (prev.type === "bos" || prev.type === "slash") {
            token.output = nodot + token.output;
          }
          push(token);
          continue;
        }
        if (prev && (prev.type === "bracket" || prev.type === "paren") && opts.regex === true) {
          token.output = value;
          push(token);
          continue;
        }
        if (state.index === state.start || prev.type === "slash" || prev.type === "dot") {
          if (prev.type === "dot") {
            state.output += NO_DOT_SLASH;
            prev.output += NO_DOT_SLASH;
          } else if (opts.dot === true) {
            state.output += NO_DOTS_SLASH;
            prev.output += NO_DOTS_SLASH;
          } else {
            state.output += nodot;
            prev.output += nodot;
          }
          if (peek() !== "*") {
            state.output += ONE_CHAR;
            prev.output += ONE_CHAR;
          }
        }
        push(token);
      }
      while (state.brackets > 0) {
        if (opts.strictBrackets === true) throw new SyntaxError(syntaxError("closing", "]"));
        state.output = utils.escapeLast(state.output, "[");
        decrement("brackets");
      }
      while (state.parens > 0) {
        if (opts.strictBrackets === true) throw new SyntaxError(syntaxError("closing", ")"));
        state.output = utils.escapeLast(state.output, "(");
        decrement("parens");
      }
      while (state.braces > 0) {
        if (opts.strictBrackets === true) throw new SyntaxError(syntaxError("closing", "}"));
        state.output = utils.escapeLast(state.output, "{");
        decrement("braces");
      }
      if (opts.strictSlashes !== true && (prev.type === "star" || prev.type === "bracket")) {
        push({ type: "maybe_slash", value: "", output: `${SLASH_LITERAL}?` });
      }
      if (state.backtrack === true) {
        state.output = "";
        for (const token of state.tokens) {
          state.output += token.output != null ? token.output : token.value;
          if (token.suffix) {
            state.output += token.suffix;
          }
        }
      }
      return state;
    };
    parse.fastpaths = (input2, options) => {
      const opts = { ...options };
      const max = typeof opts.maxLength === "number" ? Math.min(MAX_LENGTH, opts.maxLength) : MAX_LENGTH;
      const len = input2.length;
      if (len > max) {
        throw new SyntaxError(`Input length: ${len}, exceeds maximum allowed length: ${max}`);
      }
      input2 = REPLACEMENTS[input2] || input2;
      const {
        DOT_LITERAL,
        SLASH_LITERAL,
        ONE_CHAR,
        DOTS_SLASH,
        NO_DOT,
        NO_DOTS,
        NO_DOTS_SLASH,
        STAR,
        START_ANCHOR
      } = constants.globChars(opts.windows);
      const nodot = opts.dot ? NO_DOTS : NO_DOT;
      const slashDot = opts.dot ? NO_DOTS_SLASH : NO_DOT;
      const capture = opts.capture ? "" : "?:";
      const state = { negated: false, prefix: "" };
      let star = opts.bash === true ? ".*?" : STAR;
      if (opts.capture) {
        star = `(${star})`;
      }
      const globstar = (opts2) => {
        if (opts2.noglobstar === true) return star;
        return `(${capture}(?:(?!${START_ANCHOR}${opts2.dot ? DOTS_SLASH : DOT_LITERAL}).)*?)`;
      };
      const create = (str) => {
        switch (str) {
          case "*":
            return `${nodot}${ONE_CHAR}${star}`;
          case ".*":
            return `${DOT_LITERAL}${ONE_CHAR}${star}`;
          case "*.*":
            return `${nodot}${star}${DOT_LITERAL}${ONE_CHAR}${star}`;
          case "*/*":
            return `${nodot}${star}${SLASH_LITERAL}${ONE_CHAR}${slashDot}${star}`;
          case "**":
            return nodot + globstar(opts);
          case "**/*":
            return `(?:${nodot}${globstar(opts)}${SLASH_LITERAL})?${slashDot}${ONE_CHAR}${star}`;
          case "**/*.*":
            return `(?:${nodot}${globstar(opts)}${SLASH_LITERAL})?${slashDot}${star}${DOT_LITERAL}${ONE_CHAR}${star}`;
          case "**/.*":
            return `(?:${nodot}${globstar(opts)}${SLASH_LITERAL})?${DOT_LITERAL}${ONE_CHAR}${star}`;
          default: {
            const match = /^(.*?)\.(\w+)$/.exec(str);
            if (!match) return;
            const source2 = create(match[1]);
            if (!source2) return;
            return source2 + DOT_LITERAL + match[2];
          }
        }
      };
      const output2 = utils.removePrefix(input2, state);
      let source = create(output2);
      if (source && opts.strictSlashes !== true) {
        source += `${SLASH_LITERAL}?`;
      }
      return source;
    };
    module.exports = parse;
  }
});

// node_modules/picomatch/lib/picomatch.js
var require_picomatch = __commonJS({
  "node_modules/picomatch/lib/picomatch.js"(exports, module) {
    "use strict";
    var scan = require_scan();
    var parse = require_parse();
    var utils = require_utils();
    var constants = require_constants2();
    var isObject = (val) => val && typeof val === "object" && !Array.isArray(val);
    var picomatch2 = (glob, options, returnState = false) => {
      if (Array.isArray(glob)) {
        const fns = glob.map((input2) => picomatch2(input2, options, returnState));
        const arrayMatcher = (str) => {
          for (const isMatch of fns) {
            const state2 = isMatch(str);
            if (state2) return state2;
          }
          return false;
        };
        return arrayMatcher;
      }
      const isState = isObject(glob) && glob.tokens && glob.input;
      if (glob === "" || typeof glob !== "string" && !isState) {
        throw new TypeError("Expected pattern to be a non-empty string");
      }
      const opts = options || {};
      const posix = opts.windows;
      const regex = isState ? picomatch2.compileRe(glob, options) : picomatch2.makeRe(glob, options, false, true);
      const state = regex.state;
      delete regex.state;
      let isIgnored = () => false;
      if (opts.ignore) {
        const ignoreOpts = { ...options, ignore: null, onMatch: null, onResult: null };
        isIgnored = picomatch2(opts.ignore, ignoreOpts, returnState);
      }
      const matcher = (input2, returnObject = false) => {
        const { isMatch, match, output: output2 } = picomatch2.test(input2, regex, options, { glob, posix });
        const result = { glob, state, regex, posix, input: input2, output: output2, match, isMatch };
        if (typeof opts.onResult === "function") {
          opts.onResult(result);
        }
        if (isMatch === false) {
          result.isMatch = false;
          return returnObject ? result : false;
        }
        if (isIgnored(input2)) {
          if (typeof opts.onIgnore === "function") {
            opts.onIgnore(result);
          }
          result.isMatch = false;
          return returnObject ? result : false;
        }
        if (typeof opts.onMatch === "function") {
          opts.onMatch(result);
        }
        return returnObject ? result : true;
      };
      if (returnState) {
        matcher.state = state;
      }
      return matcher;
    };
    picomatch2.test = (input2, regex, options, { glob, posix } = {}) => {
      if (typeof input2 !== "string") {
        throw new TypeError("Expected input to be a string");
      }
      if (input2 === "") {
        return { isMatch: false, output: "" };
      }
      const opts = options || {};
      const format = opts.format || (posix ? utils.toPosixSlashes : null);
      let match = input2 === glob;
      let output2 = match && format ? format(input2) : input2;
      if (match === false) {
        output2 = format ? format(input2) : input2;
        match = output2 === glob;
      }
      if (match === false || opts.capture === true) {
        if (opts.matchBase === true || opts.basename === true) {
          match = picomatch2.matchBase(input2, regex, options, posix);
        } else {
          match = regex.exec(output2);
        }
      }
      return { isMatch: Boolean(match), match, output: output2 };
    };
    picomatch2.matchBase = (input2, glob, options) => {
      const regex = glob instanceof RegExp ? glob : picomatch2.makeRe(glob, options);
      return regex.test(utils.basename(input2));
    };
    picomatch2.isMatch = (str, patterns, options) => picomatch2(patterns, options)(str);
    picomatch2.parse = (pattern, options) => {
      if (Array.isArray(pattern)) return pattern.map((p) => picomatch2.parse(p, options));
      return parse(pattern, { ...options, fastpaths: false });
    };
    picomatch2.scan = (input2, options) => scan(input2, options);
    picomatch2.compileRe = (state, options, returnOutput = false, returnState = false) => {
      if (returnOutput === true) {
        return state.output;
      }
      const opts = options || {};
      const prepend = opts.contains ? "" : "^";
      const append = opts.contains ? "" : "$";
      let source = `${prepend}(?:${state.output})${append}`;
      if (state && state.negated === true) {
        source = `^(?!${source}).*$`;
      }
      const regex = picomatch2.toRegex(source, options);
      if (returnState === true) {
        regex.state = state;
      }
      return regex;
    };
    picomatch2.makeRe = (input2, options = {}, returnOutput = false, returnState = false) => {
      if (!input2 || typeof input2 !== "string") {
        throw new TypeError("Expected a non-empty string");
      }
      let parsed = { negated: false, fastpaths: true };
      if (options.fastpaths !== false && (input2[0] === "." || input2[0] === "*")) {
        parsed.output = parse.fastpaths(input2, options);
      }
      if (!parsed.output) {
        parsed = parse(input2, options);
      }
      return picomatch2.compileRe(parsed, options, returnOutput, returnState);
    };
    picomatch2.toRegex = (source, options) => {
      try {
        const opts = options || {};
        return new RegExp(source, opts.flags || (opts.nocase ? "i" : ""));
      } catch (err) {
        if (options && options.debug === true) throw err;
        return /$^/;
      }
    };
    picomatch2.constants = constants;
    module.exports = picomatch2;
  }
});

// node_modules/picomatch/index.js
var require_picomatch2 = __commonJS({
  "node_modules/picomatch/index.js"(exports, module) {
    "use strict";
    var pico = require_picomatch();
    var utils = require_utils();
    function picomatch2(glob, options, returnState = false) {
      if (options && (options.windows === null || options.windows === void 0)) {
        options = { ...options, windows: utils.isWindows() };
      }
      return pico(glob, options, returnState);
    }
    Object.assign(picomatch2, pico);
    module.exports = picomatch2;
  }
});

// node_modules/graceful-fs/polyfills.js
var require_polyfills = __commonJS({
  "node_modules/graceful-fs/polyfills.js"(exports, module) {
    var constants = __require("constants");
    var origCwd = process.cwd;
    var cwd = null;
    var platform = process.env.GRACEFUL_FS_PLATFORM || process.platform;
    process.cwd = function() {
      if (!cwd)
        cwd = origCwd.call(process);
      return cwd;
    };
    try {
      process.cwd();
    } catch (er) {
    }
    if (typeof process.chdir === "function") {
      chdir = process.chdir;
      process.chdir = function(d) {
        cwd = null;
        chdir.call(process, d);
      };
      if (Object.setPrototypeOf) Object.setPrototypeOf(process.chdir, chdir);
    }
    var chdir;
    module.exports = patch;
    function patch(fs3) {
      if (constants.hasOwnProperty("O_SYMLINK") && process.version.match(/^v0\.6\.[0-2]|^v0\.5\./)) {
        patchLchmod(fs3);
      }
      if (!fs3.lutimes) {
        patchLutimes(fs3);
      }
      fs3.chown = chownFix(fs3.chown);
      fs3.fchown = chownFix(fs3.fchown);
      fs3.lchown = chownFix(fs3.lchown);
      fs3.chmod = chmodFix(fs3.chmod);
      fs3.fchmod = chmodFix(fs3.fchmod);
      fs3.lchmod = chmodFix(fs3.lchmod);
      fs3.chownSync = chownFixSync(fs3.chownSync);
      fs3.fchownSync = chownFixSync(fs3.fchownSync);
      fs3.lchownSync = chownFixSync(fs3.lchownSync);
      fs3.chmodSync = chmodFixSync(fs3.chmodSync);
      fs3.fchmodSync = chmodFixSync(fs3.fchmodSync);
      fs3.lchmodSync = chmodFixSync(fs3.lchmodSync);
      fs3.stat = statFix(fs3.stat);
      fs3.fstat = statFix(fs3.fstat);
      fs3.lstat = statFix(fs3.lstat);
      fs3.statSync = statFixSync(fs3.statSync);
      fs3.fstatSync = statFixSync(fs3.fstatSync);
      fs3.lstatSync = statFixSync(fs3.lstatSync);
      if (fs3.chmod && !fs3.lchmod) {
        fs3.lchmod = function(path3, mode, cb) {
          if (cb) process.nextTick(cb);
        };
        fs3.lchmodSync = function() {
        };
      }
      if (fs3.chown && !fs3.lchown) {
        fs3.lchown = function(path3, uid, gid, cb) {
          if (cb) process.nextTick(cb);
        };
        fs3.lchownSync = function() {
        };
      }
      if (platform === "win32") {
        fs3.rename = typeof fs3.rename !== "function" ? fs3.rename : function(fs$rename) {
          function rename(from, to, cb) {
            var start = Date.now();
            var backoff = 0;
            fs$rename(from, to, function CB(er) {
              if (er && (er.code === "EACCES" || er.code === "EPERM" || er.code === "EBUSY") && Date.now() - start < 6e4) {
                setTimeout(function() {
                  fs3.stat(to, function(stater, st) {
                    if (stater && stater.code === "ENOENT")
                      fs$rename(from, to, CB);
                    else
                      cb(er);
                  });
                }, backoff);
                if (backoff < 100)
                  backoff += 10;
                return;
              }
              if (cb) cb(er);
            });
          }
          if (Object.setPrototypeOf) Object.setPrototypeOf(rename, fs$rename);
          return rename;
        }(fs3.rename);
      }
      fs3.read = typeof fs3.read !== "function" ? fs3.read : function(fs$read) {
        function read(fd, buffer, offset, length, position, callback_) {
          var callback;
          if (callback_ && typeof callback_ === "function") {
            var eagCounter = 0;
            callback = function(er, _, __) {
              if (er && er.code === "EAGAIN" && eagCounter < 10) {
                eagCounter++;
                return fs$read.call(fs3, fd, buffer, offset, length, position, callback);
              }
              callback_.apply(this, arguments);
            };
          }
          return fs$read.call(fs3, fd, buffer, offset, length, position, callback);
        }
        if (Object.setPrototypeOf) Object.setPrototypeOf(read, fs$read);
        return read;
      }(fs3.read);
      fs3.readSync = typeof fs3.readSync !== "function" ? fs3.readSync : /* @__PURE__ */ function(fs$readSync) {
        return function(fd, buffer, offset, length, position) {
          var eagCounter = 0;
          while (true) {
            try {
              return fs$readSync.call(fs3, fd, buffer, offset, length, position);
            } catch (er) {
              if (er.code === "EAGAIN" && eagCounter < 10) {
                eagCounter++;
                continue;
              }
              throw er;
            }
          }
        };
      }(fs3.readSync);
      function patchLchmod(fs4) {
        fs4.lchmod = function(path3, mode, callback) {
          fs4.open(
            path3,
            constants.O_WRONLY | constants.O_SYMLINK,
            mode,
            function(err, fd) {
              if (err) {
                if (callback) callback(err);
                return;
              }
              fs4.fchmod(fd, mode, function(err2) {
                fs4.close(fd, function(err22) {
                  if (callback) callback(err2 || err22);
                });
              });
            }
          );
        };
        fs4.lchmodSync = function(path3, mode) {
          var fd = fs4.openSync(path3, constants.O_WRONLY | constants.O_SYMLINK, mode);
          var threw = true;
          var ret;
          try {
            ret = fs4.fchmodSync(fd, mode);
            threw = false;
          } finally {
            if (threw) {
              try {
                fs4.closeSync(fd);
              } catch (er) {
              }
            } else {
              fs4.closeSync(fd);
            }
          }
          return ret;
        };
      }
      function patchLutimes(fs4) {
        if (constants.hasOwnProperty("O_SYMLINK") && fs4.futimes) {
          fs4.lutimes = function(path3, at, mt, cb) {
            fs4.open(path3, constants.O_SYMLINK, function(er, fd) {
              if (er) {
                if (cb) cb(er);
                return;
              }
              fs4.futimes(fd, at, mt, function(er2) {
                fs4.close(fd, function(er22) {
                  if (cb) cb(er2 || er22);
                });
              });
            });
          };
          fs4.lutimesSync = function(path3, at, mt) {
            var fd = fs4.openSync(path3, constants.O_SYMLINK);
            var ret;
            var threw = true;
            try {
              ret = fs4.futimesSync(fd, at, mt);
              threw = false;
            } finally {
              if (threw) {
                try {
                  fs4.closeSync(fd);
                } catch (er) {
                }
              } else {
                fs4.closeSync(fd);
              }
            }
            return ret;
          };
        } else if (fs4.futimes) {
          fs4.lutimes = function(_a, _b, _c, cb) {
            if (cb) process.nextTick(cb);
          };
          fs4.lutimesSync = function() {
          };
        }
      }
      function chmodFix(orig) {
        if (!orig) return orig;
        return function(target, mode, cb) {
          return orig.call(fs3, target, mode, function(er) {
            if (chownErOk(er)) er = null;
            if (cb) cb.apply(this, arguments);
          });
        };
      }
      function chmodFixSync(orig) {
        if (!orig) return orig;
        return function(target, mode) {
          try {
            return orig.call(fs3, target, mode);
          } catch (er) {
            if (!chownErOk(er)) throw er;
          }
        };
      }
      function chownFix(orig) {
        if (!orig) return orig;
        return function(target, uid, gid, cb) {
          return orig.call(fs3, target, uid, gid, function(er) {
            if (chownErOk(er)) er = null;
            if (cb) cb.apply(this, arguments);
          });
        };
      }
      function chownFixSync(orig) {
        if (!orig) return orig;
        return function(target, uid, gid) {
          try {
            return orig.call(fs3, target, uid, gid);
          } catch (er) {
            if (!chownErOk(er)) throw er;
          }
        };
      }
      function statFix(orig) {
        if (!orig) return orig;
        return function(target, options, cb) {
          if (typeof options === "function") {
            cb = options;
            options = null;
          }
          function callback(er, stats) {
            if (stats) {
              if (stats.uid < 0) stats.uid += 4294967296;
              if (stats.gid < 0) stats.gid += 4294967296;
            }
            if (cb) cb.apply(this, arguments);
          }
          return options ? orig.call(fs3, target, options, callback) : orig.call(fs3, target, callback);
        };
      }
      function statFixSync(orig) {
        if (!orig) return orig;
        return function(target, options) {
          var stats = options ? orig.call(fs3, target, options) : orig.call(fs3, target);
          if (stats) {
            if (stats.uid < 0) stats.uid += 4294967296;
            if (stats.gid < 0) stats.gid += 4294967296;
          }
          return stats;
        };
      }
      function chownErOk(er) {
        if (!er)
          return true;
        if (er.code === "ENOSYS")
          return true;
        var nonroot = !process.getuid || process.getuid() !== 0;
        if (nonroot) {
          if (er.code === "EINVAL" || er.code === "EPERM")
            return true;
        }
        return false;
      }
    }
  }
});

// node_modules/graceful-fs/legacy-streams.js
var require_legacy_streams = __commonJS({
  "node_modules/graceful-fs/legacy-streams.js"(exports, module) {
    var Stream = __require("stream").Stream;
    module.exports = legacy;
    function legacy(fs3) {
      return {
        ReadStream,
        WriteStream
      };
      function ReadStream(path3, options) {
        if (!(this instanceof ReadStream)) return new ReadStream(path3, options);
        Stream.call(this);
        var self = this;
        this.path = path3;
        this.fd = null;
        this.readable = true;
        this.paused = false;
        this.flags = "r";
        this.mode = 438;
        this.bufferSize = 64 * 1024;
        options = options || {};
        var keys = Object.keys(options);
        for (var index = 0, length = keys.length; index < length; index++) {
          var key = keys[index];
          this[key] = options[key];
        }
        if (this.encoding) this.setEncoding(this.encoding);
        if (this.start !== void 0) {
          if ("number" !== typeof this.start) {
            throw TypeError("start must be a Number");
          }
          if (this.end === void 0) {
            this.end = Infinity;
          } else if ("number" !== typeof this.end) {
            throw TypeError("end must be a Number");
          }
          if (this.start > this.end) {
            throw new Error("start must be <= end");
          }
          this.pos = this.start;
        }
        if (this.fd !== null) {
          process.nextTick(function() {
            self._read();
          });
          return;
        }
        fs3.open(this.path, this.flags, this.mode, function(err, fd) {
          if (err) {
            self.emit("error", err);
            self.readable = false;
            return;
          }
          self.fd = fd;
          self.emit("open", fd);
          self._read();
        });
      }
      function WriteStream(path3, options) {
        if (!(this instanceof WriteStream)) return new WriteStream(path3, options);
        Stream.call(this);
        this.path = path3;
        this.fd = null;
        this.writable = true;
        this.flags = "w";
        this.encoding = "binary";
        this.mode = 438;
        this.bytesWritten = 0;
        options = options || {};
        var keys = Object.keys(options);
        for (var index = 0, length = keys.length; index < length; index++) {
          var key = keys[index];
          this[key] = options[key];
        }
        if (this.start !== void 0) {
          if ("number" !== typeof this.start) {
            throw TypeError("start must be a Number");
          }
          if (this.start < 0) {
            throw new Error("start must be >= zero");
          }
          this.pos = this.start;
        }
        this.busy = false;
        this._queue = [];
        if (this.fd === null) {
          this._open = fs3.open;
          this._queue.push([this._open, this.path, this.flags, this.mode, void 0]);
          this.flush();
        }
      }
    }
  }
});

// node_modules/graceful-fs/clone.js
var require_clone = __commonJS({
  "node_modules/graceful-fs/clone.js"(exports, module) {
    "use strict";
    module.exports = clone;
    var getPrototypeOf = Object.getPrototypeOf || function(obj) {
      return obj.__proto__;
    };
    function clone(obj) {
      if (obj === null || typeof obj !== "object")
        return obj;
      if (obj instanceof Object)
        var copy = { __proto__: getPrototypeOf(obj) };
      else
        var copy = /* @__PURE__ */ Object.create(null);
      Object.getOwnPropertyNames(obj).forEach(function(key) {
        Object.defineProperty(copy, key, Object.getOwnPropertyDescriptor(obj, key));
      });
      return copy;
    }
  }
});

// node_modules/graceful-fs/graceful-fs.js
var require_graceful_fs = __commonJS({
  "node_modules/graceful-fs/graceful-fs.js"(exports, module) {
    var fs3 = __require("fs");
    var polyfills = require_polyfills();
    var legacy = require_legacy_streams();
    var clone = require_clone();
    var util = __require("util");
    var gracefulQueue;
    var previousSymbol;
    if (typeof Symbol === "function" && typeof Symbol.for === "function") {
      gracefulQueue = Symbol.for("graceful-fs.queue");
      previousSymbol = Symbol.for("graceful-fs.previous");
    } else {
      gracefulQueue = "___graceful-fs.queue";
      previousSymbol = "___graceful-fs.previous";
    }
    function noop() {
    }
    function publishQueue(context, queue2) {
      Object.defineProperty(context, gracefulQueue, {
        get: function() {
          return queue2;
        }
      });
    }
    var debug = noop;
    if (util.debuglog)
      debug = util.debuglog("gfs4");
    else if (/\bgfs4\b/i.test(process.env.NODE_DEBUG || ""))
      debug = function() {
        var m = util.format.apply(util, arguments);
        m = "GFS4: " + m.split(/\n/).join("\nGFS4: ");
        console.error(m);
      };
    if (!fs3[gracefulQueue]) {
      queue = global[gracefulQueue] || [];
      publishQueue(fs3, queue);
      fs3.close = function(fs$close) {
        function close(fd, cb) {
          return fs$close.call(fs3, fd, function(err) {
            if (!err) {
              resetQueue();
            }
            if (typeof cb === "function")
              cb.apply(this, arguments);
          });
        }
        Object.defineProperty(close, previousSymbol, {
          value: fs$close
        });
        return close;
      }(fs3.close);
      fs3.closeSync = function(fs$closeSync) {
        function closeSync(fd) {
          fs$closeSync.apply(fs3, arguments);
          resetQueue();
        }
        Object.defineProperty(closeSync, previousSymbol, {
          value: fs$closeSync
        });
        return closeSync;
      }(fs3.closeSync);
      if (/\bgfs4\b/i.test(process.env.NODE_DEBUG || "")) {
        process.on("exit", function() {
          debug(fs3[gracefulQueue]);
          __require("assert").equal(fs3[gracefulQueue].length, 0);
        });
      }
    }
    var queue;
    if (!global[gracefulQueue]) {
      publishQueue(global, fs3[gracefulQueue]);
    }
    module.exports = patch(clone(fs3));
    if (process.env.TEST_GRACEFUL_FS_GLOBAL_PATCH && !fs3.__patched) {
      module.exports = patch(fs3);
      fs3.__patched = true;
    }
    function patch(fs4) {
      polyfills(fs4);
      fs4.gracefulify = patch;
      fs4.createReadStream = createReadStream;
      fs4.createWriteStream = createWriteStream;
      var fs$readFile = fs4.readFile;
      fs4.readFile = readFile;
      function readFile(path3, options, cb) {
        if (typeof options === "function")
          cb = options, options = null;
        return go$readFile(path3, options, cb);
        function go$readFile(path4, options2, cb2, startTime) {
          return fs$readFile(path4, options2, function(err) {
            if (err && (err.code === "EMFILE" || err.code === "ENFILE"))
              enqueue([go$readFile, [path4, options2, cb2], err, startTime || Date.now(), Date.now()]);
            else {
              if (typeof cb2 === "function")
                cb2.apply(this, arguments);
            }
          });
        }
      }
      var fs$writeFile = fs4.writeFile;
      fs4.writeFile = writeFile;
      function writeFile(path3, data, options, cb) {
        if (typeof options === "function")
          cb = options, options = null;
        return go$writeFile(path3, data, options, cb);
        function go$writeFile(path4, data2, options2, cb2, startTime) {
          return fs$writeFile(path4, data2, options2, function(err) {
            if (err && (err.code === "EMFILE" || err.code === "ENFILE"))
              enqueue([go$writeFile, [path4, data2, options2, cb2], err, startTime || Date.now(), Date.now()]);
            else {
              if (typeof cb2 === "function")
                cb2.apply(this, arguments);
            }
          });
        }
      }
      var fs$appendFile = fs4.appendFile;
      if (fs$appendFile)
        fs4.appendFile = appendFile;
      function appendFile(path3, data, options, cb) {
        if (typeof options === "function")
          cb = options, options = null;
        return go$appendFile(path3, data, options, cb);
        function go$appendFile(path4, data2, options2, cb2, startTime) {
          return fs$appendFile(path4, data2, options2, function(err) {
            if (err && (err.code === "EMFILE" || err.code === "ENFILE"))
              enqueue([go$appendFile, [path4, data2, options2, cb2], err, startTime || Date.now(), Date.now()]);
            else {
              if (typeof cb2 === "function")
                cb2.apply(this, arguments);
            }
          });
        }
      }
      var fs$copyFile = fs4.copyFile;
      if (fs$copyFile)
        fs4.copyFile = copyFile;
      function copyFile(src, dest, flags, cb) {
        if (typeof flags === "function") {
          cb = flags;
          flags = 0;
        }
        return go$copyFile(src, dest, flags, cb);
        function go$copyFile(src2, dest2, flags2, cb2, startTime) {
          return fs$copyFile(src2, dest2, flags2, function(err) {
            if (err && (err.code === "EMFILE" || err.code === "ENFILE"))
              enqueue([go$copyFile, [src2, dest2, flags2, cb2], err, startTime || Date.now(), Date.now()]);
            else {
              if (typeof cb2 === "function")
                cb2.apply(this, arguments);
            }
          });
        }
      }
      var fs$readdir = fs4.readdir;
      fs4.readdir = readdir;
      var noReaddirOptionVersions = /^v[0-5]\./;
      function readdir(path3, options, cb) {
        if (typeof options === "function")
          cb = options, options = null;
        var go$readdir = noReaddirOptionVersions.test(process.version) ? function go$readdir2(path4, options2, cb2, startTime) {
          return fs$readdir(path4, fs$readdirCallback(
            path4,
            options2,
            cb2,
            startTime
          ));
        } : function go$readdir2(path4, options2, cb2, startTime) {
          return fs$readdir(path4, options2, fs$readdirCallback(
            path4,
            options2,
            cb2,
            startTime
          ));
        };
        return go$readdir(path3, options, cb);
        function fs$readdirCallback(path4, options2, cb2, startTime) {
          return function(err, files) {
            if (err && (err.code === "EMFILE" || err.code === "ENFILE"))
              enqueue([
                go$readdir,
                [path4, options2, cb2],
                err,
                startTime || Date.now(),
                Date.now()
              ]);
            else {
              if (files && files.sort)
                files.sort();
              if (typeof cb2 === "function")
                cb2.call(this, err, files);
            }
          };
        }
      }
      if (process.version.substr(0, 4) === "v0.8") {
        var legStreams = legacy(fs4);
        ReadStream = legStreams.ReadStream;
        WriteStream = legStreams.WriteStream;
      }
      var fs$ReadStream = fs4.ReadStream;
      if (fs$ReadStream) {
        ReadStream.prototype = Object.create(fs$ReadStream.prototype);
        ReadStream.prototype.open = ReadStream$open;
      }
      var fs$WriteStream = fs4.WriteStream;
      if (fs$WriteStream) {
        WriteStream.prototype = Object.create(fs$WriteStream.prototype);
        WriteStream.prototype.open = WriteStream$open;
      }
      Object.defineProperty(fs4, "ReadStream", {
        get: function() {
          return ReadStream;
        },
        set: function(val) {
          ReadStream = val;
        },
        enumerable: true,
        configurable: true
      });
      Object.defineProperty(fs4, "WriteStream", {
        get: function() {
          return WriteStream;
        },
        set: function(val) {
          WriteStream = val;
        },
        enumerable: true,
        configurable: true
      });
      var FileReadStream = ReadStream;
      Object.defineProperty(fs4, "FileReadStream", {
        get: function() {
          return FileReadStream;
        },
        set: function(val) {
          FileReadStream = val;
        },
        enumerable: true,
        configurable: true
      });
      var FileWriteStream = WriteStream;
      Object.defineProperty(fs4, "FileWriteStream", {
        get: function() {
          return FileWriteStream;
        },
        set: function(val) {
          FileWriteStream = val;
        },
        enumerable: true,
        configurable: true
      });
      function ReadStream(path3, options) {
        if (this instanceof ReadStream)
          return fs$ReadStream.apply(this, arguments), this;
        else
          return ReadStream.apply(Object.create(ReadStream.prototype), arguments);
      }
      function ReadStream$open() {
        var that = this;
        open(that.path, that.flags, that.mode, function(err, fd) {
          if (err) {
            if (that.autoClose)
              that.destroy();
            that.emit("error", err);
          } else {
            that.fd = fd;
            that.emit("open", fd);
            that.read();
          }
        });
      }
      function WriteStream(path3, options) {
        if (this instanceof WriteStream)
          return fs$WriteStream.apply(this, arguments), this;
        else
          return WriteStream.apply(Object.create(WriteStream.prototype), arguments);
      }
      function WriteStream$open() {
        var that = this;
        open(that.path, that.flags, that.mode, function(err, fd) {
          if (err) {
            that.destroy();
            that.emit("error", err);
          } else {
            that.fd = fd;
            that.emit("open", fd);
          }
        });
      }
      function createReadStream(path3, options) {
        return new fs4.ReadStream(path3, options);
      }
      function createWriteStream(path3, options) {
        return new fs4.WriteStream(path3, options);
      }
      var fs$open = fs4.open;
      fs4.open = open;
      function open(path3, flags, mode, cb) {
        if (typeof mode === "function")
          cb = mode, mode = null;
        return go$open(path3, flags, mode, cb);
        function go$open(path4, flags2, mode2, cb2, startTime) {
          return fs$open(path4, flags2, mode2, function(err, fd) {
            if (err && (err.code === "EMFILE" || err.code === "ENFILE"))
              enqueue([go$open, [path4, flags2, mode2, cb2], err, startTime || Date.now(), Date.now()]);
            else {
              if (typeof cb2 === "function")
                cb2.apply(this, arguments);
            }
          });
        }
      }
      return fs4;
    }
    function enqueue(elem) {
      debug("ENQUEUE", elem[0].name, elem[1]);
      fs3[gracefulQueue].push(elem);
      retry();
    }
    var retryTimer;
    function resetQueue() {
      var now = Date.now();
      for (var i = 0; i < fs3[gracefulQueue].length; ++i) {
        if (fs3[gracefulQueue][i].length > 2) {
          fs3[gracefulQueue][i][3] = now;
          fs3[gracefulQueue][i][4] = now;
        }
      }
      retry();
    }
    function retry() {
      clearTimeout(retryTimer);
      retryTimer = void 0;
      if (fs3[gracefulQueue].length === 0)
        return;
      var elem = fs3[gracefulQueue].shift();
      var fn = elem[0];
      var args = elem[1];
      var err = elem[2];
      var startTime = elem[3];
      var lastTime = elem[4];
      if (startTime === void 0) {
        debug("RETRY", fn.name, args);
        fn.apply(null, args);
      } else if (Date.now() - startTime >= 6e4) {
        debug("TIMEOUT", fn.name, args);
        var cb = args.pop();
        if (typeof cb === "function")
          cb.call(null, err);
      } else {
        var sinceAttempt = Date.now() - lastTime;
        var sinceStart = Math.max(lastTime - startTime, 1);
        var desiredDelay = Math.min(sinceStart * 1.2, 100);
        if (sinceAttempt >= desiredDelay) {
          debug("RETRY", fn.name, args);
          fn.apply(null, args.concat([startTime]));
        } else {
          fs3[gracefulQueue].push(elem);
        }
      }
      if (retryTimer === void 0) {
        retryTimer = setTimeout(retry, 0);
      }
    }
  }
});

// node_modules/retry/lib/retry_operation.js
var require_retry_operation = __commonJS({
  "node_modules/retry/lib/retry_operation.js"(exports, module) {
    function RetryOperation(timeouts, options) {
      if (typeof options === "boolean") {
        options = { forever: options };
      }
      this._originalTimeouts = JSON.parse(JSON.stringify(timeouts));
      this._timeouts = timeouts;
      this._options = options || {};
      this._maxRetryTime = options && options.maxRetryTime || Infinity;
      this._fn = null;
      this._errors = [];
      this._attempts = 1;
      this._operationTimeout = null;
      this._operationTimeoutCb = null;
      this._timeout = null;
      this._operationStart = null;
      if (this._options.forever) {
        this._cachedTimeouts = this._timeouts.slice(0);
      }
    }
    module.exports = RetryOperation;
    RetryOperation.prototype.reset = function() {
      this._attempts = 1;
      this._timeouts = this._originalTimeouts;
    };
    RetryOperation.prototype.stop = function() {
      if (this._timeout) {
        clearTimeout(this._timeout);
      }
      this._timeouts = [];
      this._cachedTimeouts = null;
    };
    RetryOperation.prototype.retry = function(err) {
      if (this._timeout) {
        clearTimeout(this._timeout);
      }
      if (!err) {
        return false;
      }
      var currentTime = (/* @__PURE__ */ new Date()).getTime();
      if (err && currentTime - this._operationStart >= this._maxRetryTime) {
        this._errors.unshift(new Error("RetryOperation timeout occurred"));
        return false;
      }
      this._errors.push(err);
      var timeout = this._timeouts.shift();
      if (timeout === void 0) {
        if (this._cachedTimeouts) {
          this._errors.splice(this._errors.length - 1, this._errors.length);
          this._timeouts = this._cachedTimeouts.slice(0);
          timeout = this._timeouts.shift();
        } else {
          return false;
        }
      }
      var self = this;
      var timer = setTimeout(function() {
        self._attempts++;
        if (self._operationTimeoutCb) {
          self._timeout = setTimeout(function() {
            self._operationTimeoutCb(self._attempts);
          }, self._operationTimeout);
          if (self._options.unref) {
            self._timeout.unref();
          }
        }
        self._fn(self._attempts);
      }, timeout);
      if (this._options.unref) {
        timer.unref();
      }
      return true;
    };
    RetryOperation.prototype.attempt = function(fn, timeoutOps) {
      this._fn = fn;
      if (timeoutOps) {
        if (timeoutOps.timeout) {
          this._operationTimeout = timeoutOps.timeout;
        }
        if (timeoutOps.cb) {
          this._operationTimeoutCb = timeoutOps.cb;
        }
      }
      var self = this;
      if (this._operationTimeoutCb) {
        this._timeout = setTimeout(function() {
          self._operationTimeoutCb();
        }, self._operationTimeout);
      }
      this._operationStart = (/* @__PURE__ */ new Date()).getTime();
      this._fn(this._attempts);
    };
    RetryOperation.prototype.try = function(fn) {
      console.log("Using RetryOperation.try() is deprecated");
      this.attempt(fn);
    };
    RetryOperation.prototype.start = function(fn) {
      console.log("Using RetryOperation.start() is deprecated");
      this.attempt(fn);
    };
    RetryOperation.prototype.start = RetryOperation.prototype.try;
    RetryOperation.prototype.errors = function() {
      return this._errors;
    };
    RetryOperation.prototype.attempts = function() {
      return this._attempts;
    };
    RetryOperation.prototype.mainError = function() {
      if (this._errors.length === 0) {
        return null;
      }
      var counts = {};
      var mainError = null;
      var mainErrorCount = 0;
      for (var i = 0; i < this._errors.length; i++) {
        var error = this._errors[i];
        var message = error.message;
        var count = (counts[message] || 0) + 1;
        counts[message] = count;
        if (count >= mainErrorCount) {
          mainError = error;
          mainErrorCount = count;
        }
      }
      return mainError;
    };
  }
});

// node_modules/retry/lib/retry.js
var require_retry = __commonJS({
  "node_modules/retry/lib/retry.js"(exports) {
    var RetryOperation = require_retry_operation();
    exports.operation = function(options) {
      var timeouts = exports.timeouts(options);
      return new RetryOperation(timeouts, {
        forever: options && options.forever,
        unref: options && options.unref,
        maxRetryTime: options && options.maxRetryTime
      });
    };
    exports.timeouts = function(options) {
      if (options instanceof Array) {
        return [].concat(options);
      }
      var opts = {
        retries: 10,
        factor: 2,
        minTimeout: 1 * 1e3,
        maxTimeout: Infinity,
        randomize: false
      };
      for (var key in options) {
        opts[key] = options[key];
      }
      if (opts.minTimeout > opts.maxTimeout) {
        throw new Error("minTimeout is greater than maxTimeout");
      }
      var timeouts = [];
      for (var i = 0; i < opts.retries; i++) {
        timeouts.push(this.createTimeout(i, opts));
      }
      if (options && options.forever && !timeouts.length) {
        timeouts.push(this.createTimeout(i, opts));
      }
      timeouts.sort(function(a, b) {
        return a - b;
      });
      return timeouts;
    };
    exports.createTimeout = function(attempt, opts) {
      var random = opts.randomize ? Math.random() + 1 : 1;
      var timeout = Math.round(random * opts.minTimeout * Math.pow(opts.factor, attempt));
      timeout = Math.min(timeout, opts.maxTimeout);
      return timeout;
    };
    exports.wrap = function(obj, options, methods) {
      if (options instanceof Array) {
        methods = options;
        options = null;
      }
      if (!methods) {
        methods = [];
        for (var key in obj) {
          if (typeof obj[key] === "function") {
            methods.push(key);
          }
        }
      }
      for (var i = 0; i < methods.length; i++) {
        var method = methods[i];
        var original = obj[method];
        obj[method] = function retryWrapper(original2) {
          var op = exports.operation(options);
          var args = Array.prototype.slice.call(arguments, 1);
          var callback = args.pop();
          args.push(function(err) {
            if (op.retry(err)) {
              return;
            }
            if (err) {
              arguments[0] = op.mainError();
            }
            callback.apply(this, arguments);
          });
          op.attempt(function() {
            original2.apply(obj, args);
          });
        }.bind(obj, original);
        obj[method].options = options;
      }
    };
  }
});

// node_modules/retry/index.js
var require_retry2 = __commonJS({
  "node_modules/retry/index.js"(exports, module) {
    module.exports = require_retry();
  }
});

// node_modules/signal-exit/signals.js
var require_signals = __commonJS({
  "node_modules/signal-exit/signals.js"(exports, module) {
    module.exports = [
      "SIGABRT",
      "SIGALRM",
      "SIGHUP",
      "SIGINT",
      "SIGTERM"
    ];
    if (process.platform !== "win32") {
      module.exports.push(
        "SIGVTALRM",
        "SIGXCPU",
        "SIGXFSZ",
        "SIGUSR2",
        "SIGTRAP",
        "SIGSYS",
        "SIGQUIT",
        "SIGIOT"
        // should detect profiler and enable/disable accordingly.
        // see #21
        // 'SIGPROF'
      );
    }
    if (process.platform === "linux") {
      module.exports.push(
        "SIGIO",
        "SIGPOLL",
        "SIGPWR",
        "SIGSTKFLT",
        "SIGUNUSED"
      );
    }
  }
});

// node_modules/signal-exit/index.js
var require_signal_exit = __commonJS({
  "node_modules/signal-exit/index.js"(exports, module) {
    var process2 = global.process;
    var processOk = function(process3) {
      return process3 && typeof process3 === "object" && typeof process3.removeListener === "function" && typeof process3.emit === "function" && typeof process3.reallyExit === "function" && typeof process3.listeners === "function" && typeof process3.kill === "function" && typeof process3.pid === "number" && typeof process3.on === "function";
    };
    if (!processOk(process2)) {
      module.exports = function() {
        return function() {
        };
      };
    } else {
      assert = __require("assert");
      signals = require_signals();
      isWin = /^win/i.test(process2.platform);
      EE = __require("events");
      if (typeof EE !== "function") {
        EE = EE.EventEmitter;
      }
      if (process2.__signal_exit_emitter__) {
        emitter = process2.__signal_exit_emitter__;
      } else {
        emitter = process2.__signal_exit_emitter__ = new EE();
        emitter.count = 0;
        emitter.emitted = {};
      }
      if (!emitter.infinite) {
        emitter.setMaxListeners(Infinity);
        emitter.infinite = true;
      }
      module.exports = function(cb, opts) {
        if (!processOk(global.process)) {
          return function() {
          };
        }
        assert.equal(typeof cb, "function", "a callback must be provided for exit handler");
        if (loaded === false) {
          load();
        }
        var ev = "exit";
        if (opts && opts.alwaysLast) {
          ev = "afterexit";
        }
        var remove = function() {
          emitter.removeListener(ev, cb);
          if (emitter.listeners("exit").length === 0 && emitter.listeners("afterexit").length === 0) {
            unload();
          }
        };
        emitter.on(ev, cb);
        return remove;
      };
      unload = function unload2() {
        if (!loaded || !processOk(global.process)) {
          return;
        }
        loaded = false;
        signals.forEach(function(sig) {
          try {
            process2.removeListener(sig, sigListeners[sig]);
          } catch (er) {
          }
        });
        process2.emit = originalProcessEmit;
        process2.reallyExit = originalProcessReallyExit;
        emitter.count -= 1;
      };
      module.exports.unload = unload;
      emit2 = function emit3(event, code, signal) {
        if (emitter.emitted[event]) {
          return;
        }
        emitter.emitted[event] = true;
        emitter.emit(event, code, signal);
      };
      sigListeners = {};
      signals.forEach(function(sig) {
        sigListeners[sig] = function listener() {
          if (!processOk(global.process)) {
            return;
          }
          var listeners = process2.listeners(sig);
          if (listeners.length === emitter.count) {
            unload();
            emit2("exit", null, sig);
            emit2("afterexit", null, sig);
            if (isWin && sig === "SIGHUP") {
              sig = "SIGINT";
            }
            process2.kill(process2.pid, sig);
          }
        };
      });
      module.exports.signals = function() {
        return signals;
      };
      loaded = false;
      load = function load2() {
        if (loaded || !processOk(global.process)) {
          return;
        }
        loaded = true;
        emitter.count += 1;
        signals = signals.filter(function(sig) {
          try {
            process2.on(sig, sigListeners[sig]);
            return true;
          } catch (er) {
            return false;
          }
        });
        process2.emit = processEmit;
        process2.reallyExit = processReallyExit;
      };
      module.exports.load = load;
      originalProcessReallyExit = process2.reallyExit;
      processReallyExit = function processReallyExit2(code) {
        if (!processOk(global.process)) {
          return;
        }
        process2.exitCode = code || /* istanbul ignore next */
        0;
        emit2("exit", process2.exitCode, null);
        emit2("afterexit", process2.exitCode, null);
        originalProcessReallyExit.call(process2, process2.exitCode);
      };
      originalProcessEmit = process2.emit;
      processEmit = function processEmit2(ev, arg) {
        if (ev === "exit" && processOk(global.process)) {
          if (arg !== void 0) {
            process2.exitCode = arg;
          }
          var ret = originalProcessEmit.apply(this, arguments);
          emit2("exit", process2.exitCode, null);
          emit2("afterexit", process2.exitCode, null);
          return ret;
        } else {
          return originalProcessEmit.apply(this, arguments);
        }
      };
    }
    var assert;
    var signals;
    var isWin;
    var EE;
    var emitter;
    var unload;
    var emit2;
    var sigListeners;
    var loaded;
    var load;
    var originalProcessReallyExit;
    var processReallyExit;
    var originalProcessEmit;
    var processEmit;
  }
});

// node_modules/proper-lockfile/lib/mtime-precision.js
var require_mtime_precision = __commonJS({
  "node_modules/proper-lockfile/lib/mtime-precision.js"(exports, module) {
    "use strict";
    var cacheSymbol = Symbol();
    function probe(file, fs3, callback) {
      const cachedPrecision = fs3[cacheSymbol];
      if (cachedPrecision) {
        return fs3.stat(file, (err, stat) => {
          if (err) {
            return callback(err);
          }
          callback(null, stat.mtime, cachedPrecision);
        });
      }
      const mtime = new Date(Math.ceil(Date.now() / 1e3) * 1e3 + 5);
      fs3.utimes(file, mtime, mtime, (err) => {
        if (err) {
          return callback(err);
        }
        fs3.stat(file, (err2, stat) => {
          if (err2) {
            return callback(err2);
          }
          const precision = stat.mtime.getTime() % 1e3 === 0 ? "s" : "ms";
          Object.defineProperty(fs3, cacheSymbol, { value: precision });
          callback(null, stat.mtime, precision);
        });
      });
    }
    function getMtime(precision) {
      let now = Date.now();
      if (precision === "s") {
        now = Math.ceil(now / 1e3) * 1e3;
      }
      return new Date(now);
    }
    module.exports.probe = probe;
    module.exports.getMtime = getMtime;
  }
});

// node_modules/proper-lockfile/lib/lockfile.js
var require_lockfile = __commonJS({
  "node_modules/proper-lockfile/lib/lockfile.js"(exports, module) {
    "use strict";
    var path3 = __require("path");
    var fs3 = require_graceful_fs();
    var retry = require_retry2();
    var onExit = require_signal_exit();
    var mtimePrecision = require_mtime_precision();
    var locks = {};
    function getLockFile(file, options) {
      return options.lockfilePath || `${file}.lock`;
    }
    function resolveCanonicalPath(file, options, callback) {
      if (!options.realpath) {
        return callback(null, path3.resolve(file));
      }
      options.fs.realpath(file, callback);
    }
    function acquireLock(file, options, callback) {
      const lockfilePath = getLockFile(file, options);
      options.fs.mkdir(lockfilePath, (err) => {
        if (!err) {
          return mtimePrecision.probe(lockfilePath, options.fs, (err2, mtime, mtimePrecision2) => {
            if (err2) {
              options.fs.rmdir(lockfilePath, () => {
              });
              return callback(err2);
            }
            callback(null, mtime, mtimePrecision2);
          });
        }
        if (err.code !== "EEXIST") {
          return callback(err);
        }
        if (options.stale <= 0) {
          return callback(Object.assign(new Error("Lock file is already being held"), { code: "ELOCKED", file }));
        }
        options.fs.stat(lockfilePath, (err2, stat) => {
          if (err2) {
            if (err2.code === "ENOENT") {
              return acquireLock(file, { ...options, stale: 0 }, callback);
            }
            return callback(err2);
          }
          if (!isLockStale(stat, options)) {
            return callback(Object.assign(new Error("Lock file is already being held"), { code: "ELOCKED", file }));
          }
          removeLock(file, options, (err3) => {
            if (err3) {
              return callback(err3);
            }
            acquireLock(file, { ...options, stale: 0 }, callback);
          });
        });
      });
    }
    function isLockStale(stat, options) {
      return stat.mtime.getTime() < Date.now() - options.stale;
    }
    function removeLock(file, options, callback) {
      options.fs.rmdir(getLockFile(file, options), (err) => {
        if (err && err.code !== "ENOENT") {
          return callback(err);
        }
        callback();
      });
    }
    function updateLock(file, options) {
      const lock2 = locks[file];
      if (lock2.updateTimeout) {
        return;
      }
      lock2.updateDelay = lock2.updateDelay || options.update;
      lock2.updateTimeout = setTimeout(() => {
        lock2.updateTimeout = null;
        options.fs.stat(lock2.lockfilePath, (err, stat) => {
          const isOverThreshold = lock2.lastUpdate + options.stale < Date.now();
          if (err) {
            if (err.code === "ENOENT" || isOverThreshold) {
              return setLockAsCompromised(file, lock2, Object.assign(err, { code: "ECOMPROMISED" }));
            }
            lock2.updateDelay = 1e3;
            return updateLock(file, options);
          }
          const isMtimeOurs = lock2.mtime.getTime() === stat.mtime.getTime();
          if (!isMtimeOurs) {
            return setLockAsCompromised(
              file,
              lock2,
              Object.assign(
                new Error("Unable to update lock within the stale threshold"),
                { code: "ECOMPROMISED" }
              )
            );
          }
          const mtime = mtimePrecision.getMtime(lock2.mtimePrecision);
          options.fs.utimes(lock2.lockfilePath, mtime, mtime, (err2) => {
            const isOverThreshold2 = lock2.lastUpdate + options.stale < Date.now();
            if (lock2.released) {
              return;
            }
            if (err2) {
              if (err2.code === "ENOENT" || isOverThreshold2) {
                return setLockAsCompromised(file, lock2, Object.assign(err2, { code: "ECOMPROMISED" }));
              }
              lock2.updateDelay = 1e3;
              return updateLock(file, options);
            }
            lock2.mtime = mtime;
            lock2.lastUpdate = Date.now();
            lock2.updateDelay = null;
            updateLock(file, options);
          });
        });
      }, lock2.updateDelay);
      if (lock2.updateTimeout.unref) {
        lock2.updateTimeout.unref();
      }
    }
    function setLockAsCompromised(file, lock2, err) {
      lock2.released = true;
      if (lock2.updateTimeout) {
        clearTimeout(lock2.updateTimeout);
      }
      if (locks[file] === lock2) {
        delete locks[file];
      }
      lock2.options.onCompromised(err);
    }
    function lock(file, options, callback) {
      options = {
        stale: 1e4,
        update: null,
        realpath: true,
        retries: 0,
        fs: fs3,
        onCompromised: (err) => {
          throw err;
        },
        ...options
      };
      options.retries = options.retries || 0;
      options.retries = typeof options.retries === "number" ? { retries: options.retries } : options.retries;
      options.stale = Math.max(options.stale || 0, 2e3);
      options.update = options.update == null ? options.stale / 2 : options.update || 0;
      options.update = Math.max(Math.min(options.update, options.stale / 2), 1e3);
      resolveCanonicalPath(file, options, (err, file2) => {
        if (err) {
          return callback(err);
        }
        const operation = retry.operation(options.retries);
        operation.attempt(() => {
          acquireLock(file2, options, (err2, mtime, mtimePrecision2) => {
            if (operation.retry(err2)) {
              return;
            }
            if (err2) {
              return callback(operation.mainError());
            }
            const lock2 = locks[file2] = {
              lockfilePath: getLockFile(file2, options),
              mtime,
              mtimePrecision: mtimePrecision2,
              options,
              lastUpdate: Date.now()
            };
            updateLock(file2, options);
            callback(null, (releasedCallback) => {
              if (lock2.released) {
                return releasedCallback && releasedCallback(Object.assign(new Error("Lock is already released"), { code: "ERELEASED" }));
              }
              unlock(file2, { ...options, realpath: false }, releasedCallback);
            });
          });
        });
      });
    }
    function unlock(file, options, callback) {
      options = {
        fs: fs3,
        realpath: true,
        ...options
      };
      resolveCanonicalPath(file, options, (err, file2) => {
        if (err) {
          return callback(err);
        }
        const lock2 = locks[file2];
        if (!lock2) {
          return callback(Object.assign(new Error("Lock is not acquired/owned by you"), { code: "ENOTACQUIRED" }));
        }
        lock2.updateTimeout && clearTimeout(lock2.updateTimeout);
        lock2.released = true;
        delete locks[file2];
        removeLock(file2, options, callback);
      });
    }
    function check(file, options, callback) {
      options = {
        stale: 1e4,
        realpath: true,
        fs: fs3,
        ...options
      };
      options.stale = Math.max(options.stale || 0, 2e3);
      resolveCanonicalPath(file, options, (err, file2) => {
        if (err) {
          return callback(err);
        }
        options.fs.stat(getLockFile(file2, options), (err2, stat) => {
          if (err2) {
            return err2.code === "ENOENT" ? callback(null, false) : callback(err2);
          }
          return callback(null, !isLockStale(stat, options));
        });
      });
    }
    function getLocks() {
      return locks;
    }
    onExit(() => {
      for (const file in locks) {
        const options = locks[file].options;
        try {
          options.fs.rmdirSync(getLockFile(file, options));
        } catch (e) {
        }
      }
    });
    module.exports.lock = lock;
    module.exports.unlock = unlock;
    module.exports.check = check;
    module.exports.getLocks = getLocks;
  }
});

// node_modules/proper-lockfile/lib/adapter.js
var require_adapter = __commonJS({
  "node_modules/proper-lockfile/lib/adapter.js"(exports, module) {
    "use strict";
    var fs3 = require_graceful_fs();
    function createSyncFs(fs4) {
      const methods = ["mkdir", "realpath", "stat", "rmdir", "utimes"];
      const newFs = { ...fs4 };
      methods.forEach((method) => {
        newFs[method] = (...args) => {
          const callback = args.pop();
          let ret;
          try {
            ret = fs4[`${method}Sync`](...args);
          } catch (err) {
            return callback(err);
          }
          callback(null, ret);
        };
      });
      return newFs;
    }
    function toPromise(method) {
      return (...args) => new Promise((resolve, reject) => {
        args.push((err, result) => {
          if (err) {
            reject(err);
          } else {
            resolve(result);
          }
        });
        method(...args);
      });
    }
    function toSync(method) {
      return (...args) => {
        let err;
        let result;
        args.push((_err, _result) => {
          err = _err;
          result = _result;
        });
        method(...args);
        if (err) {
          throw err;
        }
        return result;
      };
    }
    function toSyncOptions(options) {
      options = { ...options };
      options.fs = createSyncFs(options.fs || fs3);
      if (typeof options.retries === "number" && options.retries > 0 || options.retries && typeof options.retries.retries === "number" && options.retries.retries > 0) {
        throw Object.assign(new Error("Cannot use retries with the sync api"), { code: "ESYNC" });
      }
      return options;
    }
    module.exports = {
      toPromise,
      toSync,
      toSyncOptions
    };
  }
});

// node_modules/proper-lockfile/index.js
var require_proper_lockfile = __commonJS({
  "node_modules/proper-lockfile/index.js"(exports, module) {
    "use strict";
    var lockfile2 = require_lockfile();
    var { toPromise, toSync, toSyncOptions } = require_adapter();
    async function lock(file, options) {
      const release = await toPromise(lockfile2.lock)(file, options);
      return toPromise(release);
    }
    function lockSync(file, options) {
      const release = toSync(lockfile2.lock)(file, toSyncOptions(options));
      return toSync(release);
    }
    function unlock(file, options) {
      return toPromise(lockfile2.unlock)(file, options);
    }
    function unlockSync(file, options) {
      return toSync(lockfile2.unlock)(file, toSyncOptions(options));
    }
    function check(file, options) {
      return toPromise(lockfile2.check)(file, options);
    }
    function checkSync(file, options) {
      return toSync(lockfile2.check)(file, toSyncOptions(options));
    }
    module.exports = lock;
    module.exports.lock = lock;
    module.exports.unlock = unlock;
    module.exports.lockSync = lockSync;
    module.exports.unlockSync = unlockSync;
    module.exports.check = check;
    module.exports.checkSync = checkSync;
  }
});

// src/store.ts
var git = __toESM(require_isomorphic_git(), 1);
import fs from "node:fs";
import path from "node:path";
import { Buffer as Buffer2 } from "node:buffer";

// node_modules/diff/lib/index.mjs
function Diff() {
}
Diff.prototype = {
  diff: function diff(oldString, newString) {
    var _options$timeout;
    var options = arguments.length > 2 && arguments[2] !== void 0 ? arguments[2] : {};
    var callback = options.callback;
    if (typeof options === "function") {
      callback = options;
      options = {};
    }
    this.options = options;
    var self = this;
    function done(value) {
      if (callback) {
        setTimeout(function() {
          callback(void 0, value);
        }, 0);
        return true;
      } else {
        return value;
      }
    }
    oldString = this.castInput(oldString);
    newString = this.castInput(newString);
    oldString = this.removeEmpty(this.tokenize(oldString));
    newString = this.removeEmpty(this.tokenize(newString));
    var newLen = newString.length, oldLen = oldString.length;
    var editLength = 1;
    var maxEditLength = newLen + oldLen;
    if (options.maxEditLength) {
      maxEditLength = Math.min(maxEditLength, options.maxEditLength);
    }
    var maxExecutionTime = (_options$timeout = options.timeout) !== null && _options$timeout !== void 0 ? _options$timeout : Infinity;
    var abortAfterTimestamp = Date.now() + maxExecutionTime;
    var bestPath = [{
      oldPos: -1,
      lastComponent: void 0
    }];
    var newPos = this.extractCommon(bestPath[0], newString, oldString, 0);
    if (bestPath[0].oldPos + 1 >= oldLen && newPos + 1 >= newLen) {
      return done([{
        value: this.join(newString),
        count: newString.length
      }]);
    }
    var minDiagonalToConsider = -Infinity, maxDiagonalToConsider = Infinity;
    function execEditLength() {
      for (var diagonalPath = Math.max(minDiagonalToConsider, -editLength); diagonalPath <= Math.min(maxDiagonalToConsider, editLength); diagonalPath += 2) {
        var basePath = void 0;
        var removePath = bestPath[diagonalPath - 1], addPath = bestPath[diagonalPath + 1];
        if (removePath) {
          bestPath[diagonalPath - 1] = void 0;
        }
        var canAdd = false;
        if (addPath) {
          var addPathNewPos = addPath.oldPos - diagonalPath;
          canAdd = addPath && 0 <= addPathNewPos && addPathNewPos < newLen;
        }
        var canRemove = removePath && removePath.oldPos + 1 < oldLen;
        if (!canAdd && !canRemove) {
          bestPath[diagonalPath] = void 0;
          continue;
        }
        if (!canRemove || canAdd && removePath.oldPos + 1 < addPath.oldPos) {
          basePath = self.addToPath(addPath, true, void 0, 0);
        } else {
          basePath = self.addToPath(removePath, void 0, true, 1);
        }
        newPos = self.extractCommon(basePath, newString, oldString, diagonalPath);
        if (basePath.oldPos + 1 >= oldLen && newPos + 1 >= newLen) {
          return done(buildValues(self, basePath.lastComponent, newString, oldString, self.useLongestToken));
        } else {
          bestPath[diagonalPath] = basePath;
          if (basePath.oldPos + 1 >= oldLen) {
            maxDiagonalToConsider = Math.min(maxDiagonalToConsider, diagonalPath - 1);
          }
          if (newPos + 1 >= newLen) {
            minDiagonalToConsider = Math.max(minDiagonalToConsider, diagonalPath + 1);
          }
        }
      }
      editLength++;
    }
    if (callback) {
      (function exec() {
        setTimeout(function() {
          if (editLength > maxEditLength || Date.now() > abortAfterTimestamp) {
            return callback();
          }
          if (!execEditLength()) {
            exec();
          }
        }, 0);
      })();
    } else {
      while (editLength <= maxEditLength && Date.now() <= abortAfterTimestamp) {
        var ret = execEditLength();
        if (ret) {
          return ret;
        }
      }
    }
  },
  addToPath: function addToPath(path3, added, removed, oldPosInc) {
    var last = path3.lastComponent;
    if (last && last.added === added && last.removed === removed) {
      return {
        oldPos: path3.oldPos + oldPosInc,
        lastComponent: {
          count: last.count + 1,
          added,
          removed,
          previousComponent: last.previousComponent
        }
      };
    } else {
      return {
        oldPos: path3.oldPos + oldPosInc,
        lastComponent: {
          count: 1,
          added,
          removed,
          previousComponent: last
        }
      };
    }
  },
  extractCommon: function extractCommon(basePath, newString, oldString, diagonalPath) {
    var newLen = newString.length, oldLen = oldString.length, oldPos = basePath.oldPos, newPos = oldPos - diagonalPath, commonCount = 0;
    while (newPos + 1 < newLen && oldPos + 1 < oldLen && this.equals(newString[newPos + 1], oldString[oldPos + 1])) {
      newPos++;
      oldPos++;
      commonCount++;
    }
    if (commonCount) {
      basePath.lastComponent = {
        count: commonCount,
        previousComponent: basePath.lastComponent
      };
    }
    basePath.oldPos = oldPos;
    return newPos;
  },
  equals: function equals(left, right) {
    if (this.options.comparator) {
      return this.options.comparator(left, right);
    } else {
      return left === right || this.options.ignoreCase && left.toLowerCase() === right.toLowerCase();
    }
  },
  removeEmpty: function removeEmpty(array) {
    var ret = [];
    for (var i = 0; i < array.length; i++) {
      if (array[i]) {
        ret.push(array[i]);
      }
    }
    return ret;
  },
  castInput: function castInput(value) {
    return value;
  },
  tokenize: function tokenize(value) {
    return value.split("");
  },
  join: function join(chars) {
    return chars.join("");
  }
};
function buildValues(diff2, lastComponent, newString, oldString, useLongestToken) {
  var components = [];
  var nextComponent;
  while (lastComponent) {
    components.push(lastComponent);
    nextComponent = lastComponent.previousComponent;
    delete lastComponent.previousComponent;
    lastComponent = nextComponent;
  }
  components.reverse();
  var componentPos = 0, componentLen = components.length, newPos = 0, oldPos = 0;
  for (; componentPos < componentLen; componentPos++) {
    var component = components[componentPos];
    if (!component.removed) {
      if (!component.added && useLongestToken) {
        var value = newString.slice(newPos, newPos + component.count);
        value = value.map(function(value2, i) {
          var oldValue = oldString[oldPos + i];
          return oldValue.length > value2.length ? oldValue : value2;
        });
        component.value = diff2.join(value);
      } else {
        component.value = diff2.join(newString.slice(newPos, newPos + component.count));
      }
      newPos += component.count;
      if (!component.added) {
        oldPos += component.count;
      }
    } else {
      component.value = diff2.join(oldString.slice(oldPos, oldPos + component.count));
      oldPos += component.count;
      if (componentPos && components[componentPos - 1].added) {
        var tmp = components[componentPos - 1];
        components[componentPos - 1] = components[componentPos];
        components[componentPos] = tmp;
      }
    }
  }
  var finalComponent = components[componentLen - 1];
  if (componentLen > 1 && typeof finalComponent.value === "string" && (finalComponent.added || finalComponent.removed) && diff2.equals("", finalComponent.value)) {
    components[componentLen - 2].value += finalComponent.value;
    components.pop();
  }
  return components;
}
var characterDiff = new Diff();
var extendedWordChars = /^[A-Za-z\xC0-\u02C6\u02C8-\u02D7\u02DE-\u02FF\u1E00-\u1EFF]+$/;
var reWhitespace = /\S/;
var wordDiff = new Diff();
wordDiff.equals = function(left, right) {
  if (this.options.ignoreCase) {
    left = left.toLowerCase();
    right = right.toLowerCase();
  }
  return left === right || this.options.ignoreWhitespace && !reWhitespace.test(left) && !reWhitespace.test(right);
};
wordDiff.tokenize = function(value) {
  var tokens = value.split(/([^\S\r\n]+|[()[\]{}'"\r\n]|\b)/);
  for (var i = 0; i < tokens.length - 1; i++) {
    if (!tokens[i + 1] && tokens[i + 2] && extendedWordChars.test(tokens[i]) && extendedWordChars.test(tokens[i + 2])) {
      tokens[i] += tokens[i + 2];
      tokens.splice(i + 1, 2);
      i--;
    }
  }
  return tokens;
};
var lineDiff = new Diff();
lineDiff.tokenize = function(value) {
  if (this.options.stripTrailingCr) {
    value = value.replace(/\r\n/g, "\n");
  }
  var retLines = [], linesAndNewlines = value.split(/(\n|\r\n)/);
  if (!linesAndNewlines[linesAndNewlines.length - 1]) {
    linesAndNewlines.pop();
  }
  for (var i = 0; i < linesAndNewlines.length; i++) {
    var line = linesAndNewlines[i];
    if (i % 2 && !this.options.newlineIsToken) {
      retLines[retLines.length - 1] += line;
    } else {
      if (this.options.ignoreWhitespace) {
        line = line.trim();
      }
      retLines.push(line);
    }
  }
  return retLines;
};
function diffLines(oldStr, newStr, callback) {
  return lineDiff.diff(oldStr, newStr, callback);
}
var sentenceDiff = new Diff();
sentenceDiff.tokenize = function(value) {
  return value.split(/(\S.+?[.!?])(?=\s+|$)/);
};
var cssDiff = new Diff();
cssDiff.tokenize = function(value) {
  return value.split(/([{}:;,]|\s+)/);
};
function _typeof(obj) {
  "@babel/helpers - typeof";
  if (typeof Symbol === "function" && typeof Symbol.iterator === "symbol") {
    _typeof = function(obj2) {
      return typeof obj2;
    };
  } else {
    _typeof = function(obj2) {
      return obj2 && typeof Symbol === "function" && obj2.constructor === Symbol && obj2 !== Symbol.prototype ? "symbol" : typeof obj2;
    };
  }
  return _typeof(obj);
}
function _toConsumableArray(arr) {
  return _arrayWithoutHoles(arr) || _iterableToArray(arr) || _unsupportedIterableToArray(arr) || _nonIterableSpread();
}
function _arrayWithoutHoles(arr) {
  if (Array.isArray(arr)) return _arrayLikeToArray(arr);
}
function _iterableToArray(iter) {
  if (typeof Symbol !== "undefined" && Symbol.iterator in Object(iter)) return Array.from(iter);
}
function _unsupportedIterableToArray(o, minLen) {
  if (!o) return;
  if (typeof o === "string") return _arrayLikeToArray(o, minLen);
  var n = Object.prototype.toString.call(o).slice(8, -1);
  if (n === "Object" && o.constructor) n = o.constructor.name;
  if (n === "Map" || n === "Set") return Array.from(o);
  if (n === "Arguments" || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(n)) return _arrayLikeToArray(o, minLen);
}
function _arrayLikeToArray(arr, len) {
  if (len == null || len > arr.length) len = arr.length;
  for (var i = 0, arr2 = new Array(len); i < len; i++) arr2[i] = arr[i];
  return arr2;
}
function _nonIterableSpread() {
  throw new TypeError("Invalid attempt to spread non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method.");
}
var objectPrototypeToString = Object.prototype.toString;
var jsonDiff = new Diff();
jsonDiff.useLongestToken = true;
jsonDiff.tokenize = lineDiff.tokenize;
jsonDiff.castInput = function(value) {
  var _this$options = this.options, undefinedReplacement = _this$options.undefinedReplacement, _this$options$stringi = _this$options.stringifyReplacer, stringifyReplacer = _this$options$stringi === void 0 ? function(k, v) {
    return typeof v === "undefined" ? undefinedReplacement : v;
  } : _this$options$stringi;
  return typeof value === "string" ? value : JSON.stringify(canonicalize(value, null, null, stringifyReplacer), stringifyReplacer, "  ");
};
jsonDiff.equals = function(left, right) {
  return Diff.prototype.equals.call(jsonDiff, left.replace(/,([\r\n])/g, "$1"), right.replace(/,([\r\n])/g, "$1"));
};
function canonicalize(obj, stack, replacementStack, replacer, key) {
  stack = stack || [];
  replacementStack = replacementStack || [];
  if (replacer) {
    obj = replacer(key, obj);
  }
  var i;
  for (i = 0; i < stack.length; i += 1) {
    if (stack[i] === obj) {
      return replacementStack[i];
    }
  }
  var canonicalizedObj;
  if ("[object Array]" === objectPrototypeToString.call(obj)) {
    stack.push(obj);
    canonicalizedObj = new Array(obj.length);
    replacementStack.push(canonicalizedObj);
    for (i = 0; i < obj.length; i += 1) {
      canonicalizedObj[i] = canonicalize(obj[i], stack, replacementStack, replacer, key);
    }
    stack.pop();
    replacementStack.pop();
    return canonicalizedObj;
  }
  if (obj && obj.toJSON) {
    obj = obj.toJSON();
  }
  if (_typeof(obj) === "object" && obj !== null) {
    stack.push(obj);
    canonicalizedObj = {};
    replacementStack.push(canonicalizedObj);
    var sortedKeys = [], _key;
    for (_key in obj) {
      if (obj.hasOwnProperty(_key)) {
        sortedKeys.push(_key);
      }
    }
    sortedKeys.sort();
    for (i = 0; i < sortedKeys.length; i += 1) {
      _key = sortedKeys[i];
      canonicalizedObj[_key] = canonicalize(obj[_key], stack, replacementStack, replacer, _key);
    }
    stack.pop();
    replacementStack.pop();
  } else {
    canonicalizedObj = obj;
  }
  return canonicalizedObj;
}
var arrayDiff = new Diff();
arrayDiff.tokenize = function(value) {
  return value.slice();
};
arrayDiff.join = arrayDiff.removeEmpty = function(value) {
  return value;
};
function structuredPatch(oldFileName, newFileName, oldStr, newStr, oldHeader, newHeader, options) {
  if (!options) {
    options = {};
  }
  if (typeof options.context === "undefined") {
    options.context = 4;
  }
  var diff2 = diffLines(oldStr, newStr, options);
  if (!diff2) {
    return;
  }
  diff2.push({
    value: "",
    lines: []
  });
  function contextLines(lines) {
    return lines.map(function(entry) {
      return " " + entry;
    });
  }
  var hunks = [];
  var oldRangeStart = 0, newRangeStart = 0, curRange = [], oldLine = 1, newLine = 1;
  var _loop = function _loop2(i2) {
    var current = diff2[i2], lines = current.lines || current.value.replace(/\n$/, "").split("\n");
    current.lines = lines;
    if (current.added || current.removed) {
      var _curRange;
      if (!oldRangeStart) {
        var prev = diff2[i2 - 1];
        oldRangeStart = oldLine;
        newRangeStart = newLine;
        if (prev) {
          curRange = options.context > 0 ? contextLines(prev.lines.slice(-options.context)) : [];
          oldRangeStart -= curRange.length;
          newRangeStart -= curRange.length;
        }
      }
      (_curRange = curRange).push.apply(_curRange, _toConsumableArray(lines.map(function(entry) {
        return (current.added ? "+" : "-") + entry;
      })));
      if (current.added) {
        newLine += lines.length;
      } else {
        oldLine += lines.length;
      }
    } else {
      if (oldRangeStart) {
        if (lines.length <= options.context * 2 && i2 < diff2.length - 2) {
          var _curRange2;
          (_curRange2 = curRange).push.apply(_curRange2, _toConsumableArray(contextLines(lines)));
        } else {
          var _curRange3;
          var contextSize = Math.min(lines.length, options.context);
          (_curRange3 = curRange).push.apply(_curRange3, _toConsumableArray(contextLines(lines.slice(0, contextSize))));
          var hunk = {
            oldStart: oldRangeStart,
            oldLines: oldLine - oldRangeStart + contextSize,
            newStart: newRangeStart,
            newLines: newLine - newRangeStart + contextSize,
            lines: curRange
          };
          if (i2 >= diff2.length - 2 && lines.length <= options.context) {
            var oldEOFNewline = /\n$/.test(oldStr);
            var newEOFNewline = /\n$/.test(newStr);
            var noNlBeforeAdds = lines.length == 0 && curRange.length > hunk.oldLines;
            if (!oldEOFNewline && noNlBeforeAdds && oldStr.length > 0) {
              curRange.splice(hunk.oldLines, 0, "\\ No newline at end of file");
            }
            if (!oldEOFNewline && !noNlBeforeAdds || !newEOFNewline) {
              curRange.push("\\ No newline at end of file");
            }
          }
          hunks.push(hunk);
          oldRangeStart = 0;
          newRangeStart = 0;
          curRange = [];
        }
      }
      oldLine += lines.length;
      newLine += lines.length;
    }
  };
  for (var i = 0; i < diff2.length; i++) {
    _loop(i);
  }
  return {
    oldFileName,
    newFileName,
    oldHeader,
    newHeader,
    hunks
  };
}
function formatPatch(diff2) {
  if (Array.isArray(diff2)) {
    return diff2.map(formatPatch).join("\n");
  }
  var ret = [];
  if (diff2.oldFileName == diff2.newFileName) {
    ret.push("Index: " + diff2.oldFileName);
  }
  ret.push("===================================================================");
  ret.push("--- " + diff2.oldFileName + (typeof diff2.oldHeader === "undefined" ? "" : "	" + diff2.oldHeader));
  ret.push("+++ " + diff2.newFileName + (typeof diff2.newHeader === "undefined" ? "" : "	" + diff2.newHeader));
  for (var i = 0; i < diff2.hunks.length; i++) {
    var hunk = diff2.hunks[i];
    if (hunk.oldLines === 0) {
      hunk.oldStart -= 1;
    }
    if (hunk.newLines === 0) {
      hunk.newStart -= 1;
    }
    ret.push("@@ -" + hunk.oldStart + "," + hunk.oldLines + " +" + hunk.newStart + "," + hunk.newLines + " @@");
    ret.push.apply(ret, hunk.lines);
  }
  return ret.join("\n") + "\n";
}
function createTwoFilesPatch(oldFileName, newFileName, oldStr, newStr, oldHeader, newHeader, options) {
  return formatPatch(structuredPatch(oldFileName, newFileName, oldStr, newStr, oldHeader, newHeader, options));
}
function createPatch(fileName, oldStr, newStr, oldHeader, newHeader, options) {
  return createTwoFilesPatch(fileName, fileName, oldStr, newStr, oldHeader, newHeader, options);
}

// node_modules/node-diff3/dist/diff3.mjs
function LCS(buffer1, buffer2) {
  let equivalenceClasses = {};
  for (let j = 0; j < buffer2.length; j++) {
    const item = buffer2[j];
    if (equivalenceClasses[item]) {
      equivalenceClasses[item].push(j);
    } else {
      equivalenceClasses[item] = [j];
    }
  }
  const NULLRESULT = { buffer1index: -1, buffer2index: -1, chain: null };
  let candidates = [NULLRESULT];
  for (let i = 0; i < buffer1.length; i++) {
    const item = buffer1[i];
    const buffer2indices = equivalenceClasses[item] || [];
    let r = 0;
    let c = candidates[0];
    for (let jx = 0; jx < buffer2indices.length; jx++) {
      const j = buffer2indices[jx];
      let s;
      for (s = r; s < candidates.length; s++) {
        if (candidates[s].buffer2index < j && (s === candidates.length - 1 || candidates[s + 1].buffer2index > j)) {
          break;
        }
      }
      if (s < candidates.length) {
        const newCandidate = { buffer1index: i, buffer2index: j, chain: candidates[s] };
        if (r === candidates.length) {
          candidates.push(c);
        } else {
          candidates[r] = c;
        }
        r = s + 1;
        c = newCandidate;
        if (r === candidates.length) {
          break;
        }
      }
    }
    candidates[r] = c;
  }
  return candidates[candidates.length - 1];
}
function diffIndices(buffer1, buffer2) {
  const lcs = LCS(buffer1, buffer2);
  let result = [];
  let tail1 = buffer1.length;
  let tail2 = buffer2.length;
  for (let candidate = lcs; candidate !== null; candidate = candidate.chain) {
    const mismatchLength1 = tail1 - candidate.buffer1index - 1;
    const mismatchLength2 = tail2 - candidate.buffer2index - 1;
    tail1 = candidate.buffer1index;
    tail2 = candidate.buffer2index;
    if (mismatchLength1 || mismatchLength2) {
      result.push({
        buffer1: [tail1 + 1, mismatchLength1],
        buffer1Content: buffer1.slice(tail1 + 1, tail1 + 1 + mismatchLength1),
        buffer2: [tail2 + 1, mismatchLength2],
        buffer2Content: buffer2.slice(tail2 + 1, tail2 + 1 + mismatchLength2)
      });
    }
  }
  result.reverse();
  return result;
}
function diff3MergeRegions(a, o, b) {
  let hunks = [];
  function addHunk(h, ab) {
    hunks.push({
      ab,
      oStart: h.buffer1[0],
      oLength: h.buffer1[1],
      abStart: h.buffer2[0],
      abLength: h.buffer2[1]
    });
  }
  diffIndices(o, a).forEach((item) => addHunk(item, "a"));
  diffIndices(o, b).forEach((item) => addHunk(item, "b"));
  hunks.sort((x, y) => x.oStart - y.oStart);
  let results = [];
  let currOffset = 0;
  function advanceTo(endOffset) {
    if (endOffset > currOffset) {
      results.push({
        stable: true,
        buffer: "o",
        bufferStart: currOffset,
        bufferLength: endOffset - currOffset,
        bufferContent: o.slice(currOffset, endOffset)
      });
      currOffset = endOffset;
    }
  }
  while (hunks.length) {
    let hunk = hunks.shift();
    let regionStart = hunk.oStart;
    let regionEnd = hunk.oStart + hunk.oLength;
    let regionHunks = [hunk];
    advanceTo(regionStart);
    while (hunks.length) {
      const nextHunk = hunks[0];
      const nextHunkStart = nextHunk.oStart;
      if (nextHunkStart > regionEnd)
        break;
      regionEnd = Math.max(regionEnd, nextHunkStart + nextHunk.oLength);
      regionHunks.push(hunks.shift());
    }
    if (regionHunks.length === 1) {
      if (hunk.abLength > 0) {
        const buffer = hunk.ab === "a" ? a : b;
        results.push({
          stable: true,
          buffer: hunk.ab,
          bufferStart: hunk.abStart,
          bufferLength: hunk.abLength,
          bufferContent: buffer.slice(hunk.abStart, hunk.abStart + hunk.abLength)
        });
      }
    } else {
      let bounds = {
        a: [a.length, -1, o.length, -1],
        b: [b.length, -1, o.length, -1]
      };
      while (regionHunks.length) {
        hunk = regionHunks.shift();
        const oStart = hunk.oStart;
        const oEnd = oStart + hunk.oLength;
        const abStart = hunk.abStart;
        const abEnd = abStart + hunk.abLength;
        let b2 = bounds[hunk.ab];
        b2[0] = Math.min(abStart, b2[0]);
        b2[1] = Math.max(abEnd, b2[1]);
        b2[2] = Math.min(oStart, b2[2]);
        b2[3] = Math.max(oEnd, b2[3]);
      }
      const aStart = bounds.a[0] + (regionStart - bounds.a[2]);
      const aEnd = bounds.a[1] + (regionEnd - bounds.a[3]);
      const bStart = bounds.b[0] + (regionStart - bounds.b[2]);
      const bEnd = bounds.b[1] + (regionEnd - bounds.b[3]);
      let result = {
        stable: false,
        aStart,
        aLength: aEnd - aStart,
        aContent: a.slice(aStart, aEnd),
        oStart: regionStart,
        oLength: regionEnd - regionStart,
        oContent: o.slice(regionStart, regionEnd),
        bStart,
        bLength: bEnd - bStart,
        bContent: b.slice(bStart, bEnd)
      };
      results.push(result);
    }
    currOffset = regionEnd;
  }
  advanceTo(o.length);
  return results;
}
function diff3Merge(a, o, b, options) {
  let defaults = {
    excludeFalseConflicts: true,
    stringSeparator: /\s+/
  };
  options = Object.assign(defaults, options);
  if (typeof a === "string")
    a = a.split(options.stringSeparator);
  if (typeof o === "string")
    o = o.split(options.stringSeparator);
  if (typeof b === "string")
    b = b.split(options.stringSeparator);
  let results = [];
  const regions = diff3MergeRegions(a, o, b);
  let okBuffer = [];
  function flushOk() {
    if (okBuffer.length) {
      results.push({ ok: okBuffer });
    }
    okBuffer = [];
  }
  function isFalseConflict(a2, b2) {
    if (a2.length !== b2.length)
      return false;
    for (let i = 0; i < a2.length; i++) {
      if (a2[i] !== b2[i])
        return false;
    }
    return true;
  }
  regions.forEach((region) => {
    if (region.stable) {
      okBuffer.push(...region.bufferContent);
    } else {
      if (options.excludeFalseConflicts && isFalseConflict(region.aContent, region.bContent)) {
        okBuffer.push(...region.aContent);
      } else {
        flushOk();
        results.push({
          conflict: {
            a: region.aContent,
            aIndex: region.aStart,
            o: region.oContent,
            oIndex: region.oStart,
            b: region.bContent,
            bIndex: region.bStart
          }
        });
      }
    }
  });
  flushOk();
  return results;
}
function merge(a, o, b, options) {
  const defaults = {
    excludeFalseConflicts: true,
    stringSeparator: /\s+/,
    label: {}
  };
  options = Object.assign(defaults, options);
  const aSection = "<<<<<<<" + (options.label.a ? ` ${options.label.a}` : "");
  const xSection = "=======";
  const bSection = ">>>>>>>" + (options.label.b ? ` ${options.label.b}` : "");
  const regions = diff3Merge(a, o, b, options);
  let conflict = false;
  let result = [];
  regions.forEach((region) => {
    if (region.ok) {
      result = result.concat(region.ok);
    } else if (region.conflict) {
      conflict = true;
      result = result.concat([aSection], region.conflict.a, [xSection], region.conflict.b, [bSection]);
    }
  });
  return {
    conflict,
    result
  };
}

// src/store.ts
var MODE_BLOB = "100644";
var MODE_TREE = "040000";
var AUTHOR = { name: "git-fs", email: "git-fs@local" };
var decoder = new TextDecoder("utf-8", { fatal: true });
var Store = class _Store {
  gitdir;
  constructor(gitdir) {
    this.gitdir = gitdir;
  }
  static async open(repo) {
    if (!fs.existsSync(path.join(repo, "HEAD"))) {
      throw new Error(`Cannot open repo '${repo}'. Run 'git-fs init' first.`);
    }
    return new _Store(repo);
  }
  static async init(repo) {
    await fs.promises.mkdir(repo, { recursive: true });
    await git.init({ fs, dir: repo, gitdir: repo, bare: true });
    return new _Store(repo);
  }
  // ── helpers ───────────────────────────────────────────────────────────────
  static branchRef(branch) {
    return branch.startsWith("refs/") ? branch : `refs/heads/${branch}`;
  }
  static withSessionTrailer(msg, branch) {
    const session = branch.startsWith("agent/") ? branch.slice("agent/".length) : null;
    if (!session) return msg;
    if (msg.includes("Session-Id:")) return msg;
    const trimmed = msg.replace(/\n+$/, "");
    if (trimmed.length === 0) return `Session-Id: ${session}
`;
    return `${trimmed}

Session-Id: ${session}
`;
  }
  now() {
    return {
      ...AUTHOR,
      timestamp: Math.floor(Date.now() / 1e3),
      timezoneOffset: (/* @__PURE__ */ new Date()).getTimezoneOffset()
    };
  }
  async resolveCommitOid(ref) {
    try {
      return await git.resolveRef({ fs, gitdir: this.gitdir, ref });
    } catch {
      return await git.expandOid({ fs, gitdir: this.gitdir, oid: ref });
    }
  }
  async resolveTreeOid(ref) {
    const commitOid = await this.resolveCommitOid(ref);
    const { commit } = await git.readCommit({ fs, gitdir: this.gitdir, oid: commitOid });
    return commit.tree;
  }
  async tryReadRefCommit(refname) {
    try {
      return await git.resolveRef({ fs, gitdir: this.gitdir, ref: refname });
    } catch {
      return null;
    }
  }
  // ── branch ────────────────────────────────────────────────────────────────
  async branchCreate(name, from) {
    if (from) {
      const oid = await this.resolveCommitOid(from);
      await git.writeRef({
        fs,
        gitdir: this.gitdir,
        ref: _Store.branchRef(name),
        value: oid,
        force: false
      });
      return;
    }
    const treeOid = await git.writeTree({ fs, gitdir: this.gitdir, tree: [] });
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: "init",
        tree: treeOid,
        parent: [],
        author: sig,
        committer: sig
      }
    });
    await git.writeRef({
      fs,
      gitdir: this.gitdir,
      ref: _Store.branchRef(name),
      value: commitOid,
      force: false
    });
  }
  async branchList() {
    return git.listBranches({ fs, gitdir: this.gitdir });
  }
  async branchDelete(name) {
    await git.deleteBranch({ fs, gitdir: this.gitdir, ref: name });
  }
  // ── file ops ──────────────────────────────────────────────────────────────
  async writeFile(branch, filepath, content, msg) {
    const refname = _Store.branchRef(branch);
    const blobOid = await git.writeBlob({ fs, gitdir: this.gitdir, blob: content });
    const parentOid = await this.tryReadRefCommit(refname);
    const baseTreeOid = parentOid ? (await git.readCommit({ fs, gitdir: this.gitdir, oid: parentOid })).commit.tree : null;
    const newTreeOid = await this.treeInsert(baseTreeOid, filepath, blobOid);
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: _Store.withSessionTrailer(msg, branch),
        tree: newTreeOid,
        parent: parentOid ? [parentOid] : [],
        author: sig,
        committer: sig
      }
    });
    await git.writeRef({
      fs,
      gitdir: this.gitdir,
      ref: refname,
      value: commitOid,
      force: true
    });
    return commitOid;
  }
  async readFile(ref, filepath) {
    const treeOid = await this.resolveTreeOid(ref);
    const { blob } = await git.readBlob({ fs, gitdir: this.gitdir, oid: treeOid, filepath });
    return blob;
  }
  async readText(ref, filepath) {
    try {
      return decoder.decode(await this.readFile(ref, filepath));
    } catch {
      throw new Error(`'${filepath}' is not valid UTF-8`);
    }
  }
  async readFileLines(ref, filepath, start, end) {
    const text = await this.readText(ref, filepath);
    const lines = text.split("\n");
    if (lines.length > 0 && lines[lines.length - 1] === "" && text.endsWith("\n")) {
      lines.pop();
    }
    const total = lines.length;
    const s = Math.max(0, (start ?? 1) - 1);
    const e = Math.min(total, end ?? total);
    if (s >= e && total > 0) {
      throw new Error(`start_line ${start} >= end_line ${end}`);
    }
    return lines.slice(s, e).join("\n");
  }
  async patchFile(branch, filepath, startLine, endLine, newContent, msg) {
    const text = await this.readText(branch, filepath);
    const hadTrailingNewline = text.endsWith("\n");
    const lines = text.split("\n");
    if (lines.length > 0 && lines[lines.length - 1] === "" && hadTrailingNewline) {
      lines.pop();
    }
    const s = Math.max(0, startLine - 1);
    const e = Math.min(lines.length, endLine);
    if (s > e) throw new Error(`start_line ${startLine} > end_line ${endLine}`);
    const replacement = newContent.split("\n");
    if (replacement.length > 0 && replacement[replacement.length - 1] === "" && newContent.endsWith("\n")) {
      replacement.pop();
    }
    lines.splice(s, e - s, ...replacement);
    let result = lines.join("\n");
    if (hadTrailingNewline) result += "\n";
    return this.writeFile(branch, filepath, Buffer2.from(result, "utf-8"), msg);
  }
  async replaceFile(branch, filepath, oldStr, newStr, msg) {
    const text = await this.readText(branch, filepath);
    const idx = text.indexOf(oldStr);
    if (idx < 0) throw new Error(`old_str not found in '${filepath}'`);
    if (text.indexOf(oldStr, idx + oldStr.length) >= 0) {
      let count = 1;
      let from = idx + oldStr.length;
      while (true) {
        const next = text.indexOf(oldStr, from);
        if (next < 0) break;
        count++;
        from = next + oldStr.length;
      }
      throw new Error(
        `old_str matches ${count} locations in '${filepath}'; make it unique by including more context`
      );
    }
    const result = text.slice(0, idx) + newStr + text.slice(idx + oldStr.length);
    return this.writeFile(branch, filepath, Buffer2.from(result, "utf-8"), msg);
  }
  async removeFile(branch, filepath, msg) {
    const refname = _Store.branchRef(branch);
    const parentOid = await this.tryReadRefCommit(refname);
    if (!parentOid) throw new Error(`branch '${branch}' has no commits`);
    const baseTreeOid = (await git.readCommit({ fs, gitdir: this.gitdir, oid: parentOid })).commit.tree;
    const newTreeOid = await this.treeRemove(baseTreeOid, filepath);
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: _Store.withSessionTrailer(msg, branch),
        tree: newTreeOid,
        parent: [parentOid],
        author: sig,
        committer: sig
      }
    });
    await git.writeRef({ fs, gitdir: this.gitdir, ref: refname, value: commitOid, force: true });
    return commitOid;
  }
  async listFiles(ref, prefix, recursive) {
    const rootTreeOid = await this.resolveTreeOid(ref);
    let scopeTreeOid = rootTreeOid;
    if (prefix.length > 0) {
      const { tree } = await git.readTree({ fs, gitdir: this.gitdir, oid: rootTreeOid, filepath: prefix });
      scopeTreeOid = await git.writeTree({ fs, gitdir: this.gitdir, tree });
    }
    const out = [];
    await this.walkTree(scopeTreeOid, prefix, recursive, out);
    return out;
  }
  async walkTree(treeOid, prefix, recursive, out) {
    const { tree } = await git.readTree({ fs, gitdir: this.gitdir, oid: treeOid });
    for (const entry of tree) {
      const full = prefix.length === 0 ? entry.path : `${prefix}/${entry.path}`;
      if (entry.type === "blob") {
        const { object } = await git.readObject({ fs, gitdir: this.gitdir, oid: entry.oid });
        const size = object.byteLength;
        out.push({ path: full, kind: "blob", size, oid: entry.oid });
      } else if (entry.type === "tree") {
        if (recursive) {
          await this.walkTree(entry.oid, full, recursive, out);
        } else {
          out.push({ path: full, kind: "tree", size: 0, oid: entry.oid });
        }
      }
    }
  }
  // ── merge / log / diff / checkout ─────────────────────────────────────────
  /**
   * 3-way merge of `base/ours/theirs` commit refs. When both sides diverge
   * from base on the same blob a line-level diff3 merge is attempted; if
   * that still has conflicting hunks the path is reported as a conflict and
   * `ours` is preserved in the tree.
   */
  async merge(baseRef, oursRef, theirsRef, into, msg) {
    const baseCommit = await this.resolveCommitOid(baseRef);
    const oursCommit = await this.resolveCommitOid(oursRef);
    const theirsCommit = await this.resolveCommitOid(theirsRef);
    const baseTree = (await git.readCommit({ fs, gitdir: this.gitdir, oid: baseCommit })).commit.tree;
    const ourTree = (await git.readCommit({ fs, gitdir: this.gitdir, oid: oursCommit })).commit.tree;
    const theirTree = (await git.readCommit({ fs, gitdir: this.gitdir, oid: theirsCommit })).commit.tree;
    const merged = await this.mergeTrees(baseTree, ourTree, theirTree);
    if (merged.conflicts.length > 0) {
      return { kind: "conflicts", conflicts: merged.conflicts };
    }
    if (!into) return { kind: "clean", treeOid: merged.treeOid, commitOid: null };
    const targetRef = _Store.branchRef(into);
    const targetTip = await this.tryReadRefCommit(targetRef);
    const parents = targetTip === theirsCommit ? [theirsCommit, oursCommit] : [oursCommit, theirsCommit];
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: _Store.withSessionTrailer(msg, oursRef),
        tree: merged.treeOid,
        parent: parents,
        author: sig,
        committer: sig
      }
    });
    await git.writeRef({ fs, gitdir: this.gitdir, ref: targetRef, value: commitOid, force: true });
    return { kind: "clean", treeOid: merged.treeOid, commitOid };
  }
  async mergeTrees(baseTreeOid, ourTreeOid, theirTreeOid) {
    const conflicts = [];
    const treeOid = await this.mergeTreeRec(baseTreeOid, ourTreeOid, theirTreeOid, "", conflicts);
    return { treeOid, conflicts };
  }
  async mergeTreeRec(baseTreeOid, ourTreeOid, theirTreeOid, pathPrefix, conflicts) {
    const base = baseTreeOid ? await this.readTreeMap(baseTreeOid) : /* @__PURE__ */ new Map();
    const ours = ourTreeOid ? await this.readTreeMap(ourTreeOid) : /* @__PURE__ */ new Map();
    const theirs = theirTreeOid ? await this.readTreeMap(theirTreeOid) : /* @__PURE__ */ new Map();
    const names = /* @__PURE__ */ new Set([...base.keys(), ...ours.keys(), ...theirs.keys()]);
    const out = [];
    for (const name of names) {
      const b = base.get(name);
      const o = ours.get(name);
      const t = theirs.get(name);
      const full = pathPrefix.length === 0 ? name : `${pathPrefix}/${name}`;
      const types = new Set(
        [b, o, t].filter((x) => !!x).map((x) => x.type)
      );
      if (types.size > 1) {
        conflicts.push({
          path: full,
          ancestor: await this.maybeBlobText(b),
          ours: await this.maybeBlobText(o),
          theirs: await this.maybeBlobText(t)
        });
        if (o) out.push(o);
        continue;
      }
      const onlyType = [...types][0];
      if (onlyType === "tree") {
        const subOid = await this.mergeTreeRec(
          b?.oid ?? null,
          o?.oid ?? null,
          t?.oid ?? null,
          full,
          conflicts
        );
        out.push({ mode: MODE_TREE, path: name, oid: subOid, type: "tree" });
        continue;
      }
      const oOid = o?.oid;
      const tOid = t?.oid;
      const bOid = b?.oid;
      if (oOid === tOid) {
        if (o) out.push(o);
        continue;
      }
      if (oOid === bOid) {
        if (t) out.push(t);
        continue;
      }
      if (tOid === bOid) {
        if (o) out.push(o);
        continue;
      }
      const lineMerge = await this.tryLineMerge(b, o, t);
      if (lineMerge.clean) {
        const mergedOid = await git.writeBlob({
          fs,
          gitdir: this.gitdir,
          blob: Buffer2.from(lineMerge.text, "utf-8")
        });
        out.push({ mode: MODE_BLOB, path: name, oid: mergedOid, type: "blob" });
        continue;
      }
      conflicts.push({
        path: full,
        ancestor: await this.maybeBlobText(b),
        ours: await this.maybeBlobText(o),
        theirs: await this.maybeBlobText(t)
      });
      if (o) out.push(o);
    }
    out.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
    return git.writeTree({ fs, gitdir: this.gitdir, tree: out });
  }
  async readTreeMap(oid) {
    const { tree } = await git.readTree({ fs, gitdir: this.gitdir, oid });
    const m = /* @__PURE__ */ new Map();
    for (const e of tree) m.set(e.path, e);
    return m;
  }
  /**
   * 3-way line merge. clean=true when no hunks overlap; false when either
   * side touches the same lines or any side is non-text.
   */
  async tryLineMerge(base, ours, theirs) {
    if (!base || !ours || !theirs) return { clean: false };
    if (base.type !== "blob" || ours.type !== "blob" || theirs.type !== "blob") {
      return { clean: false };
    }
    const baseText = await this.maybeBlobText(base);
    const ourText = await this.maybeBlobText(ours);
    const theirText = await this.maybeBlobText(theirs);
    if (baseText === null || ourText === null || theirText === null) return { clean: false };
    try {
      const merged = merge(ourText, baseText, theirText, { stringSeparator: /\r?\n/ });
      if (merged.conflict) return { clean: false };
      return { clean: true, text: merged.result.join("\n") };
    } catch {
      return { clean: false };
    }
  }
  async maybeBlobText(entry) {
    if (!entry || entry.type !== "blob") return null;
    try {
      const { blob } = await git.readBlob({ fs, gitdir: this.gitdir, oid: entry.oid });
      return decoder.decode(blob);
    } catch {
      return null;
    }
  }
  async log(ref, count) {
    const commits = await git.log({ fs, gitdir: this.gitdir, ref, depth: count });
    return commits.map((c) => ({
      oid: c.oid,
      message: c.commit.message.trim(),
      author: c.commit.author.name,
      time: c.commit.author.timestamp
    }));
  }
  async diff(refA, refB) {
    const treeA = await this.resolveTreeOid(refA);
    const treeB = await this.resolveTreeOid(refB);
    const changes = await this.diffTrees(treeA, treeB, "");
    return changes.join("");
  }
  async diffTrees(treeA, treeB, pathPrefix) {
    const a = await this.readTreeMap(treeA);
    const b = await this.readTreeMap(treeB);
    const names = /* @__PURE__ */ new Set([...a.keys(), ...b.keys()]);
    const out = [];
    const sorted = [...names].sort();
    for (const name of sorted) {
      const ea = a.get(name);
      const eb = b.get(name);
      const full = pathPrefix.length === 0 ? name : `${pathPrefix}/${name}`;
      if (ea && eb && ea.oid === eb.oid) continue;
      if (ea?.type === "tree" || eb?.type === "tree") {
        out.push(...await this.diffTrees(
          ea?.oid ?? "4b825dc642cb6eb9a060e54bf8d69288fbee4904",
          eb?.oid ?? "4b825dc642cb6eb9a060e54bf8d69288fbee4904",
          full
        ));
        continue;
      }
      const aText = ea ? await this.maybeBlobText(ea) : null;
      const bText = eb ? await this.maybeBlobText(eb) : null;
      out.push(unifiedDiff(full, aText, bText));
    }
    return out;
  }
  async checkout(ref, dest) {
    const treeOid = await this.resolveTreeOid(ref);
    await fs.promises.mkdir(dest, { recursive: true });
    await this.extractTree(treeOid, dest);
  }
  async extractTree(treeOid, dest) {
    const { tree } = await git.readTree({ fs, gitdir: this.gitdir, oid: treeOid });
    for (const entry of tree) {
      const p = path.join(dest, entry.path);
      if (entry.type === "blob") {
        const { blob } = await git.readBlob({ fs, gitdir: this.gitdir, oid: entry.oid });
        await fs.promises.writeFile(p, blob);
      } else if (entry.type === "tree") {
        await fs.promises.mkdir(p, { recursive: true });
        await this.extractTree(entry.oid, p);
      }
    }
  }
  // ── tree helpers ──────────────────────────────────────────────────────────
  async treeInsert(baseTreeOid, filepath, blobOid) {
    const idx = filepath.indexOf("/");
    const head = idx < 0 ? filepath : filepath.slice(0, idx);
    const tail = idx < 0 ? null : filepath.slice(idx + 1);
    const base = baseTreeOid ? await this.readTreeMap(baseTreeOid) : /* @__PURE__ */ new Map();
    if (tail === null) {
      base.set(head, { mode: MODE_BLOB, path: head, oid: blobOid, type: "blob" });
    } else {
      const sub = base.get(head);
      const subBaseOid = sub?.type === "tree" ? sub.oid : null;
      const newSubOid = await this.treeInsert(subBaseOid, tail, blobOid);
      base.set(head, { mode: MODE_TREE, path: head, oid: newSubOid, type: "tree" });
    }
    const arr = [...base.values()].sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
    return git.writeTree({ fs, gitdir: this.gitdir, tree: arr });
  }
  async treeRemove(baseTreeOid, filepath) {
    const idx = filepath.indexOf("/");
    const head = idx < 0 ? filepath : filepath.slice(0, idx);
    const tail = idx < 0 ? null : filepath.slice(idx + 1);
    const base = await this.readTreeMap(baseTreeOid);
    if (tail === null) {
      if (!base.has(head)) throw new Error(`File '${filepath}' not found`);
      base.delete(head);
    } else {
      const sub = base.get(head);
      if (!sub || sub.type !== "tree") throw new Error(`File '${filepath}' not found`);
      const newSubOid = await this.treeRemove(sub.oid, tail);
      base.set(head, { mode: MODE_TREE, path: head, oid: newSubOid, type: "tree" });
    }
    const arr = [...base.values()].sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
    return git.writeTree({ fs, gitdir: this.gitdir, tree: arr });
  }
  // ── extra helpers needed by hooks ─────────────────────────────────────────
  async resolveCommit(ref) {
    return this.resolveCommitOid(ref);
  }
  /** Force-set a ref's value (bootstrap main from agent etc). */
  async writeBranchRef(branch, oid) {
    await git.writeRef({
      fs,
      gitdir: this.gitdir,
      ref: _Store.branchRef(branch),
      value: oid,
      force: true
    });
  }
  /**
   * Remove many paths in one commit. Missing paths are silently skipped.
   * Returns the new commit OID, or null when `paths` is empty / all missing.
   */
  async removePaths(branch, paths, msg) {
    if (paths.length === 0) return null;
    const refname = _Store.branchRef(branch);
    const parentOid = await this.tryReadRefCommit(refname);
    if (!parentOid) return null;
    let treeOid = (await git.readCommit({ fs, gitdir: this.gitdir, oid: parentOid })).commit.tree;
    let removed = 0;
    for (const p of paths) {
      try {
        treeOid = await this.treeRemove(treeOid, p);
        removed++;
      } catch {
      }
    }
    if (removed === 0) return null;
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: _Store.withSessionTrailer(msg, branch),
        tree: treeOid,
        parent: [parentOid],
        author: sig,
        committer: sig
      }
    });
    await git.writeRef({ fs, gitdir: this.gitdir, ref: refname, value: commitOid, force: true });
    return commitOid;
  }
  /** Set of paths that differ between two refs (add/modify/delete). */
  async changedPaths(fromRef, toRef) {
    const fromTree = await this.resolveTreeOid(fromRef);
    const toTree = await this.resolveTreeOid(toRef);
    const out = [];
    await this.collectChanged(fromTree, toTree, "", out);
    out.sort();
    return Array.from(new Set(out));
  }
  async collectChanged(a, b, prefix, out) {
    if (a === b) return;
    const am = await this.readTreeMap(a);
    const bm = await this.readTreeMap(b);
    const names = /* @__PURE__ */ new Set([...am.keys(), ...bm.keys()]);
    for (const name of names) {
      const ea = am.get(name);
      const eb = bm.get(name);
      const full = prefix.length === 0 ? name : `${prefix}/${name}`;
      if (ea?.oid === eb?.oid) continue;
      if (ea?.type === "tree" && eb?.type === "tree") {
        await this.collectChanged(ea.oid, eb.oid, full, out);
        continue;
      }
      out.push(full);
    }
  }
  async tipTime(ref) {
    const oid = await this.resolveCommitOid(ref);
    const { commit } = await git.readCommit({ fs, gitdir: this.gitdir, oid });
    return commit.author.timestamp;
  }
  async mergeBase(aRef, bRef) {
    try {
      const a = await this.resolveCommitOid(aRef);
      const b = await this.resolveCommitOid(bRef);
      const bases = await git.findMergeBase({ fs, gitdir: this.gitdir, oids: [a, b] });
      return bases[0] ?? null;
    } catch {
      return null;
    }
  }
  async isMergedInto(branch, into) {
    try {
      const a = await this.resolveCommitOid(branch);
      const b = await this.resolveCommitOid(into);
      if (a === b) return true;
      return await git.isDescendent({ fs, gitdir: this.gitdir, oid: b, ancestor: a, depth: -1 });
    } catch {
      return false;
    }
  }
  // expose for trailer test
  static __withSessionTrailerForTest(msg, branch) {
    return _Store.withSessionTrailer(msg, branch);
  }
};
function unifiedDiff(filepath, a, b) {
  const patch = createPatch(filepath, a ?? "", b ?? "", "", "", { context: 3 });
  return patch.replace(/^Index:.*\n=+\n/, "");
}

// src/hooks.ts
var import_picomatch = __toESM(require_picomatch2(), 1);
var import_proper_lockfile = __toESM(require_proper_lockfile(), 1);
import fs2 from "node:fs";
import path2 from "node:path";
var HARD_MERGEIGNORE = [".agent", "CONFLICTS.md"];
function readStdinSync() {
  try {
    return fs2.readFileSync(0, "utf-8");
  } catch {
    return "";
  }
}
function relPath(abs, cwd) {
  const a = path2.resolve(abs);
  const c = path2.resolve(cwd);
  const rel = path2.relative(c, a);
  if (rel.startsWith("..") || path2.isAbsolute(rel)) return null;
  return rel.split(path2.sep).join("/");
}
function makeIgnoreMatcher(patterns) {
  if (patterns.length === 0) return () => false;
  const expanded = patterns.map((raw) => {
    const trimmed = raw.trim();
    if (trimmed.endsWith("/")) return `${trimmed.slice(0, -1)}/**`;
    return trimmed;
  });
  const matchers = expanded.map(
    (p) => (0, import_picomatch.default)(p, { dot: true, nocase: process.platform === "win32" })
  );
  return (target) => matchers.some((m) => m(target));
}
async function loadMergeignore(store, ref) {
  const patterns = [...HARD_MERGEIGNORE];
  try {
    const bytes = await store.readFile(ref, ".git-fs/mergeignore");
    const text = new TextDecoder("utf-8").decode(bytes);
    for (const raw of text.split(/\r?\n/)) {
      const line = raw.trim();
      if (line.length === 0 || line.startsWith("#")) continue;
      patterns.push(line);
    }
  } catch {
  }
  return makeIgnoreMatcher(patterns);
}
async function stripMergeignored(store, branch, match) {
  const entries = await store.listFiles(branch, "", true);
  const paths = entries.filter((e) => e.kind === "blob" && match(e.path)).map((e) => e.path);
  if (paths.length === 0) return 0;
  await store.removePaths(branch, paths, `strip mergeignored (${paths.length} path(s))`);
  return paths.length;
}
async function reportSiblingOverlap(store, ours, match) {
  let ourPaths;
  try {
    ourPaths = new Set((await store.changedPaths("main", ours)).filter((p) => !match(p)));
  } catch {
    return;
  }
  if (ourPaths.size === 0) return;
  const branches = await store.branchList();
  for (const sib of branches) {
    if (sib === ours || !sib.startsWith("agent/")) continue;
    let sibPaths;
    try {
      sibPaths = await store.changedPaths("main", sib);
    } catch {
      continue;
    }
    const overlap = sibPaths.filter((p) => !match(p) && ourPaths.has(p));
    if (overlap.length === 0) continue;
    process.stderr.write(`git-fs: sibling overlap with ${sib} on ${overlap.length} path(s):
`);
    for (const p of overlap) process.stderr.write(`  ${p}
`);
  }
}
async function readAgentMeta(store, branch) {
  try {
    const bytes = await store.readFile(branch, ".agent");
    const text = new TextDecoder("utf-8").decode(bytes);
    const meta = {};
    for (const line of text.split(/\r?\n/)) {
      const m = /^(\w+):\s*(.*)$/.exec(line);
      if (m) meta[m[1]] = m[2].trim();
    }
    return {
      model: meta["model"] ?? "unknown",
      session: meta["session"] ?? "",
      base: meta["base"] ?? ""
    };
  } catch {
    return null;
  }
}
async function writeAgentMeta(store, branch, meta, msg) {
  const body = `model: ${meta.model}
session: ${meta.session}
base: ${meta.base}
`;
  await store.writeFile(branch, ".agent", Buffer.from(body, "utf-8"), msg);
}
async function syncFromMain(store, branch) {
  try {
    const branches = await store.branchList();
    if (!branches.includes("main") || !branches.includes(branch)) return;
    if (await store.isMergedInto("main", branch)) return;
    const base = await store.mergeBase(branch, "main");
    if (!base) return;
    const result = await store.merge(base, branch, "main", branch, "sync from main");
    if (result.kind === "conflicts") {
      process.stderr.write(
        `git-fs: sync from main \u2192 ${branch} has ${result.conflicts.length} conflict(s); continuing with agent state
`
      );
    } else if (result.commitOid) {
      process.stderr.write(`git-fs: synced main \u2192 ${branch}
`);
    }
  } catch (e) {
    process.stderr.write(`git-fs: sync from main failed: ${e instanceof Error ? e.message : String(e)}
`);
  }
}
async function withMergeLock(repo, fn) {
  const lockPath = path2.join(repo, "merge.lock");
  if (!fs2.existsSync(lockPath)) fs2.writeFileSync(lockPath, "");
  const release = await import_proper_lockfile.default.lock(lockPath, { retries: { retries: 50, minTimeout: 50, maxTimeout: 200 } });
  try {
    return await fn();
  } finally {
    await release();
  }
}
async function runHook(action) {
  const repo = process.env["GIT_FS_REPO"] ?? ".git-fs";
  if (!fs2.existsSync(repo)) return;
  const cwd = process.cwd();
  const raw = readStdinSync();
  let payload = {};
  try {
    payload = JSON.parse(raw);
  } catch {
    return;
  }
  const sessionId = payload.session_id;
  if (typeof sessionId !== "string") return;
  const branch = `agent/${sessionId}`;
  const store = await Store.open(repo);
  switch (action) {
    case "session-start": {
      const model = payload.model ?? "unknown";
      let baseOid = "";
      let from;
      try {
        baseOid = await store.resolveCommit("main");
        from = "main";
      } catch {
        from = void 0;
      }
      try {
        await store.branchCreate(branch, from);
      } catch {
      }
      await store.writeFile(
        branch,
        ".agent",
        Buffer.from(`model: ${model}
session: ${sessionId}
base: ${baseOid}
`, "utf-8"),
        `session start

model: ${model}`
      );
      const intent = `# Session intent

session: ${sessionId}
model: ${model}

## What this session is doing


## Why


## Paths touched

(see .git-fs/session/paths.md)
`;
      try {
        await store.writeFile(branch, ".git-fs/session/intent.md", Buffer.from(intent, "utf-8"), "seed session intent");
      } catch {
      }
      process.stderr.write(`git-fs: branch '${branch}' ready (model: ${model})
`);
      process.stdout.write(
        `git-fs session active.
Branch: ${branch}
All file changes are tracked automatically via hooks.
Prefer git_fs_replace for edits (text-match, immune to line-number drift).
Use git_fs_write for new files, git_fs_read to read, git_fs_patch for line-range edits.
Use git_fs_log branch:${branch} to see your change history.
Use git_fs_branch_list to see all agent sessions.
Load schemas upfront: ToolSearch select:git_fs_write,git_fs_read,git_fs_replace,git_fs_patch,git_fs_ls,git_fs_rm,git_fs_merge,git_fs_diff,git_fs_log,git_fs_branch_create,git_fs_branch_list,git_fs_branch_delete,git_fs_checkout
`
      );
      return;
    }
    case "post-write":
    case "post-edit": {
      const filePath = payload.tool_input?.file_path;
      if (!filePath) return;
      const rel = relPath(filePath, cwd);
      if (!rel) return;
      if (!fs2.existsSync(filePath)) return;
      await syncFromMain(store, branch);
      const content = fs2.readFileSync(filePath);
      const op = action === "post-write" ? "write" : "edit";
      try {
        await store.writeFile(branch, rel, content, `${op} ${rel}`);
        process.stderr.write(`git-fs: ${op} ${branch}:${rel}
`);
      } catch (e) {
        process.stderr.write(`git-fs: ${op} ${rel} failed: ${e instanceof Error ? e.message : String(e)}
`);
      }
      return;
    }
    case "read": {
      const filePath = payload.tool_input?.file_path;
      if (!filePath) return;
      const rel = relPath(filePath, cwd);
      if (!rel) return;
      await syncFromMain(store, branch);
      try {
        const bytes = await store.readFile(branch, rel);
        const parent = path2.dirname(filePath);
        fs2.mkdirSync(parent, { recursive: true });
        fs2.writeFileSync(filePath, bytes);
      } catch {
      }
      return;
    }
    case "stop": {
      const branches = await store.branchList();
      if (!branches.includes("main")) {
        try {
          const agentOid = await store.resolveCommit(branch);
          await store.writeBranchRef("main", agentOid);
          try {
            await store.checkout("main", cwd);
            process.stderr.write(`git-fs: bootstrapped main from ${branch}; materialized \u2192 ${cwd}
`);
          } catch (e) {
            process.stderr.write(`git-fs: checkout failed: ${e instanceof Error ? e.message : String(e)}
`);
          }
        } catch (e) {
          process.stderr.write(`git-fs: bootstrap failed: ${e instanceof Error ? e.message : String(e)}
`);
        }
        return;
      }
      let base = branch;
      try {
        const bytes = await store.readFile(branch, ".agent");
        const text = new TextDecoder("utf-8").decode(bytes);
        const m = text.split(/\r?\n/).find((l) => l.startsWith("base: "));
        if (m) {
          const v = m.slice("base: ".length).trim();
          if (v.length > 0) base = v;
        }
      } catch {
      }
      const ignore = await loadMergeignore(store, "main");
      try {
        const n = await stripMergeignored(store, branch, ignore);
        if (n > 0) process.stderr.write(`git-fs: stripped ${n} mergeignored path(s) from ${branch}
`);
      } catch (e) {
        process.stderr.write(`git-fs: strip mergeignored failed: ${e instanceof Error ? e.message : String(e)}
`);
      }
      try {
        await reportSiblingOverlap(store, branch, ignore);
      } catch (e) {
        process.stderr.write(`git-fs: sibling reconcile failed: ${e instanceof Error ? e.message : String(e)}
`);
      }
      try {
        await withMergeLock(repo, async () => {
          const result = await store.merge(base, branch, "main", "main", `merge from ${branch}`);
          if (result.kind === "clean") {
            if (result.commitOid) process.stderr.write(`git-fs: merged ${branch} \u2192 main
`);
            try {
              await store.checkout("main", cwd);
              process.stderr.write(`git-fs: materialized main \u2192 ${cwd}
`);
            } catch (e) {
              process.stderr.write(`git-fs: checkout failed: ${e instanceof Error ? e.message : String(e)}
`);
            }
            try {
              const mainOid = await store.resolveCommit("main");
              const prior = await readAgentMeta(store, branch);
              await store.writeBranchRef(branch, mainOid);
              await writeAgentMeta(
                store,
                branch,
                {
                  model: prior?.model ?? "unknown",
                  session: sessionId,
                  base: mainOid
                },
                "reset to main after merge"
              );
              process.stderr.write(`git-fs: reset ${branch} \u2192 main (base ${mainOid.slice(0, 7)})
`);
            } catch (e) {
              process.stderr.write(`git-fs: reset ${branch} failed: ${e instanceof Error ? e.message : String(e)}
`);
            }
          } else {
            let report = `# Conflicts: ${branch} \u2192 main

`;
            for (const c of result.conflicts) {
              report += `## ${c.path}

ours:
\`\`\`
${c.ours ?? "(deleted)"}
\`\`\`

theirs:
\`\`\`
${c.theirs ?? "(deleted)"}
\`\`\`

`;
            }
            try {
              await store.writeFile(branch, "CONFLICTS.md", Buffer.from(report, "utf-8"), "conflict report");
            } catch {
            }
            process.stderr.write(`git-fs: conflicts on merge ${branch} \u2192 main; see CONFLICTS.md
`);
          }
        });
      } catch (e) {
        process.stderr.write(`git-fs: merge lock failed: ${e instanceof Error ? e.message : String(e)}
`);
      }
      return;
    }
  }
}
function parseDurationSecs(input2) {
  const s = input2.trim();
  if (s.length === 0) throw new Error("empty duration");
  const last = s.charAt(s.length - 1);
  if (/[0-9]/.test(last)) {
    const n = Number(s);
    if (!Number.isFinite(n)) throw new Error(`bad duration '${s}'`);
    return n;
  }
  const num = Number(s.slice(0, -1));
  if (!Number.isFinite(num)) throw new Error(`bad duration '${s}'`);
  switch (last) {
    case "s":
      return num;
    case "m":
      return num * 60;
    case "h":
      return num * 3600;
    case "d":
      return num * 86400;
    case "w":
      return num * 86400 * 7;
    default:
      throw new Error(`unknown duration unit '${last}' in '${s}'`);
  }
}
async function initProject(repoPath, mcpConfigPath) {
  if (fs2.existsSync(repoPath)) {
    try {
      await Store.open(repoPath);
    } catch {
      await Store.init(repoPath);
    }
  } else {
    await Store.init(repoPath);
  }
  const repoAbs = path2.resolve(repoPath);
  let cfg = {};
  if (fs2.existsSync(mcpConfigPath)) {
    try {
      cfg = JSON.parse(fs2.readFileSync(mcpConfigPath, "utf-8"));
    } catch {
      cfg = {};
    }
  }
  const servers = cfg["mcpServers"] ?? {};
  servers["git-fs"] = {
    command: "git-fs-mcp",
    env: { GIT_FS_REPO: repoAbs.split(path2.sep).join("/") }
  };
  cfg["mcpServers"] = servers;
  fs2.writeFileSync(mcpConfigPath, JSON.stringify(cfg, null, 2) + "\n");
  process.stdout.write(`Repo:     ${repoAbs}
`);
  process.stdout.write(`Config:   ${mcpConfigPath}

`);
  process.stdout.write(`Restart Claude Code to load the MCP server.
`);
  process.stdout.write(`Tools: git_fs_write, git_fs_read, git_fs_ls, git_fs_merge, git_fs_diff, git_fs_log ...
`);
}
async function runPrune(repoPath, opts) {
  const store = await Store.open(repoPath);
  const cutoff = opts.olderThan ? parseDurationSecs(opts.olderThan) : null;
  const now = Math.floor(Date.now() / 1e3);
  const branches = await store.branchList();
  const report = [];
  for (const b of branches) {
    if (!b.startsWith(opts.prefix) || b === opts.into) continue;
    let age;
    try {
      age = now - await store.tipTime(b);
    } catch {
      continue;
    }
    if (cutoff !== null && age < cutoff) continue;
    let merged = false;
    try {
      merged = await store.isMergedInto(b, opts.into);
    } catch {
      merged = false;
    }
    if (opts.merged && !merged) continue;
    report.push({ branch: b, ageSecs: age, merged });
  }
  const deleted = [];
  if (opts.apply) {
    for (const row of report) {
      try {
        await store.branchDelete(row.branch);
        deleted.push(row.branch);
      } catch (e) {
        process.stderr.write(`prune: failed to delete ${row.branch}: ${e instanceof Error ? e.message : String(e)}
`);
      }
    }
  }
  if (opts.json) {
    process.stdout.write(JSON.stringify({
      applied: opts.apply,
      candidates: report.map((r) => ({
        branch: r.branch,
        age_secs: r.ageSecs,
        merged: r.merged,
        deleted: opts.apply && deleted.includes(r.branch)
      }))
    }) + "\n");
    return;
  }
  if (report.length === 0) {
    process.stdout.write("nothing to prune\n");
    return;
  }
  const verb = opts.apply ? "deleted" : "would delete";
  process.stdout.write(`${verb} ${report.length} branch(es):
`);
  for (const r of report) {
    const tag = r.merged ? "merged" : "UNMERGED";
    process.stdout.write(`  ${r.branch}  age=${r.ageSecs}s  ${tag}
`);
  }
  if (!opts.apply) process.stdout.write("\n(dry run \u2014 pass --apply to delete)\n");
}

// src/cli.ts
import { Buffer as Buffer3 } from "node:buffer";
import { stdin as input, stdout as output } from "node:process";
function parseArgv(argv) {
  const positional = [];
  const flags = {};
  let repo = process.env["GIT_FS_REPO"] ?? ".git-fs";
  let json = false;
  let cmd = null;
  let sub = null;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--repo") {
      repo = argv[++i] ?? repo;
    } else if (a.startsWith("--repo=")) {
      repo = a.slice("--repo=".length);
    } else if (a === "--json") {
      json = true;
    } else if (a.startsWith("--")) {
      const eq = a.indexOf("=");
      if (eq >= 0) {
        flags[a.slice(2, eq)] = a.slice(eq + 1);
      } else {
        const next = argv[i + 1];
        if (next !== void 0 && !next.startsWith("--")) {
          flags[a.slice(2)] = next;
          i++;
        } else {
          flags[a.slice(2)] = true;
        }
      }
    } else if (cmd === null) {
      cmd = a;
    } else if ((cmd === "branch" || cmd === "hook") && sub === null) {
      sub = a;
    } else {
      positional.push(a);
    }
  }
  return { cmd: cmd ?? "", sub, rest: { repo, json, positional, flags } };
}
function emit(json, jsonValue, plain) {
  output.write(json ? JSON.stringify(jsonValue) + "\n" : plain.endsWith("\n") ? plain : plain + "\n");
}
async function readStdin() {
  const chunks = [];
  for await (const c of input) chunks.push(c);
  return Buffer3.concat(chunks);
}
async function main() {
  const { cmd, sub, rest } = parseArgv(process.argv.slice(2));
  const { repo, json, positional: p, flags: f } = rest;
  if (cmd === "" || cmd === "--help" || cmd === "-h") {
    output.write(USAGE);
    return;
  }
  if (cmd === "init") {
    await Store.init(repo);
    emit(json, { ok: true, repo }, `Initialized bare repo: ${repo}`);
    return;
  }
  if (cmd === "init-project") {
    const r = typeof f["repo"] === "string" ? f["repo"] : ".git-fs";
    const cfg = typeof f["mcp-config"] === "string" ? f["mcp-config"] : ".mcp.json";
    await initProject(r, cfg);
    return;
  }
  if (cmd === "hook") {
    process.env["GIT_FS_REPO"] = repo;
    if (sub === null) throw new Error("hook needs a subcommand");
    const allowed = ["session-start", "post-write", "post-edit", "read", "stop"];
    if (!allowed.includes(sub)) {
      throw new Error(`unknown hook: ${sub}`);
    }
    await runHook(sub);
    return;
  }
  if (cmd === "prune") {
    await runPrune(repo, {
      merged: f["merged"] === true,
      olderThan: typeof f["older-than"] === "string" ? f["older-than"] : null,
      into: typeof f["into"] === "string" ? f["into"] : "main",
      prefix: typeof f["prefix"] === "string" ? f["prefix"] : "agent/",
      apply: f["apply"] === true,
      json
    });
    return;
  }
  const store = await Store.open(repo);
  switch (cmd) {
    case "branch": {
      if (sub === "create") {
        const name = req(p, 0, "name");
        await store.branchCreate(name, typeof f["from"] === "string" ? f["from"] : void 0);
        emit(json, { ok: true, branch: name }, `Created branch: ${name}`);
      } else if (sub === "list") {
        const bs = await store.branchList();
        emit(json, bs, bs.join("\n"));
      } else if (sub === "delete") {
        const name = req(p, 0, "name");
        await store.branchDelete(name);
        emit(json, { ok: true, deleted: name }, `Deleted: ${name}`);
      } else {
        throw new Error(`unknown branch subcommand: ${sub}`);
      }
      break;
    }
    case "write": {
      const branch = req(p, 0, "branch");
      const path3 = req(p, 1, "path");
      const message = typeof f["message"] === "string" ? f["message"] : "write";
      const content = await readStdin();
      const oid = await store.writeFile(branch, path3, content, message);
      emit(json, { ok: true, commit: oid, branch, path: path3 }, `${oid.slice(0, 8)} ${branch}:${path3}`);
      break;
    }
    case "read": {
      const ref = req(p, 0, "ref");
      const path3 = req(p, 1, "path");
      const bytes = await store.readFile(ref, path3);
      output.write(bytes);
      break;
    }
    case "rm": {
      const branch = req(p, 0, "branch");
      const path3 = req(p, 1, "path");
      const msg = typeof f["message"] === "string" && f["message"] || `rm ${path3}`;
      const oid = await store.removeFile(branch, path3, msg);
      emit(json, { ok: true, commit: oid }, `${oid.slice(0, 8)} rm ${path3}`);
      break;
    }
    case "ls": {
      const ref = req(p, 0, "ref");
      const path3 = p[1] ?? "";
      const recursive = f["recursive"] === true;
      const entries = await store.listFiles(ref, path3, recursive);
      if (json) {
        output.write(JSON.stringify(entries) + "\n");
      } else {
        for (const e of entries) {
          if (e.kind === "blob") {
            output.write(`${String(e.size).padStart(8)}B  ${e.oid.slice(0, 8)}  ${e.path}
`);
          } else {
            output.write(`     dir  ${e.oid.slice(0, 8)}  ${e.path}/
`);
          }
        }
      }
      break;
    }
    case "merge": {
      const ours = req(p, 0, "ours");
      const theirs = req(p, 1, "theirs");
      const base = String(f["base"] ?? throwIf("--base required"));
      const into = typeof f["into"] === "string" ? f["into"] : null;
      const message = typeof f["message"] === "string" ? f["message"] : "merge";
      const result = await store.merge(base, ours, theirs, into, message);
      if (result.kind === "clean") {
        if (json) {
          output.write(JSON.stringify({ ok: true, tree: result.treeOid, commit: result.commitOid }) + "\n");
        } else if (result.commitOid) {
          output.write(`merged tree ${result.treeOid.slice(0, 8)} \u2192 commit ${result.commitOid.slice(0, 8)}
`);
        } else {
          output.write(`tree ${result.treeOid.slice(0, 8)} (pass --into <branch> to commit)
`);
        }
      } else {
        if (json) {
          output.write(JSON.stringify({ ok: false, conflicts: result.conflicts }) + "\n");
        } else {
          process.stderr.write(`CONFLICTS (${result.conflicts.length}):
`);
          for (const c of result.conflicts) process.stderr.write(`  conflict: ${c.path}
`);
        }
        process.exit(2);
      }
      break;
    }
    case "diff": {
      const refA = req(p, 0, "ref_a");
      const refB = req(p, 1, "ref_b");
      output.write(await store.diff(refA, refB));
      break;
    }
    case "log": {
      const ref = req(p, 0, "ref");
      const count = Number(f["count"] ?? 10);
      const entries = await store.log(ref, count);
      if (json) {
        output.write(JSON.stringify(entries) + "\n");
      } else {
        for (const e of entries) output.write(`${e.oid.slice(0, 8)} ${e.message}
`);
      }
      break;
    }
    case "checkout": {
      const ref = req(p, 0, "ref");
      const dest = req(p, 1, "dest");
      await store.checkout(ref, dest);
      emit(json, { ok: true, dest }, `Checked out '${ref}' \u2192 ${dest}`);
      break;
    }
    case "show": {
      const ref = req(p, 0, "ref");
      const entries = await store.log(ref, 1);
      const e = entries[0];
      if (!e) break;
      if (json) {
        output.write(JSON.stringify(e) + "\n");
      } else {
        output.write(`commit ${e.oid}
author ${e.author}

    ${e.message}
`);
      }
      break;
    }
    default:
      throw new Error(`unknown command: ${cmd}`);
  }
}
function req(arr, i, label) {
  const v = arr[i];
  if (typeof v !== "string") throw new Error(`${label} required`);
  return v;
}
function throwIf(msg) {
  throw new Error(msg);
}
var USAGE = `git-fs \u2014 virtual filesystem over git object store

USAGE:
  git-fs [--repo PATH] [--json] <COMMAND>

COMMANDS:
  init                                  Initialize a bare repo
  init-project [--repo P] [--mcp-config P]  Bootstrap project (bare repo + .mcp.json)
  branch create NAME [--from REF]       Create branch
  branch list                           List branches
  branch delete NAME                    Delete branch
  write BRANCH PATH [--message MSG]     Write stdin to file (auto-commits)
  read REF PATH                         Stream file to stdout
  rm BRANCH PATH [--message MSG]        Remove file
  ls REF [PATH] [--recursive]           List files
  merge OURS THEIRS --base REF [--into BRANCH] [--message MSG]
  diff REF_A REF_B                      Unified diff
  log REF [--count N]                   Commit log
  checkout REF DEST                     Materialize ref to disk
  show REF                              Show tip commit
  prune [--merged] [--older-than D] [--into REF] [--prefix P] [--apply]
  hook (session-start|post-write|post-edit|read|stop)
                                        Claude Code hook handlers
`;
main().catch((e) => {
  process.stderr.write(`error: ${e instanceof Error ? e.message : String(e)}
`);
  process.exit(1);
});
/*! Bundled license information:

safe-buffer/index.js:
  (*! safe-buffer. MIT License. Feross Aboukhadijeh <https://feross.org/opensource> *)

crc-32/crc32.js:
  (*! crc32.js (C) 2014-present SheetJS -- http://sheetjs.com *)

isomorphic-git/index.cjs:
  (*!
   * This code for `path.join` is directly copied from @zenfs/core/path for bundle size improvements.
   * SPDX-License-Identifier: LGPL-3.0-or-later
   * Copyright (c) James Prevett and other ZenFS contributors.
   *)
*/
