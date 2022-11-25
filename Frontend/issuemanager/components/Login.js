import React from "react";
import Head from "next/head";
import { FaGithub, FaGoogle, FaLinkedinIn } from "react-icons/fa";

function Login({title}) {
  return (
    <>
      <Head>
        <title>
            {title ? title + ' - GitCore' : 'GitCore'}
          <link rel="icon" href="favicon.ico" type="image/x-icon" />
        </title>
      </Head>
      <div className="flex items-center justify-center min-h-screen py-2 bg-gray-100">
        <main className="flex flex-col items-center justify-center w-full flex-1 px-20 text-center">
          <div className="bg-white rounded-2xl shadow-2xl w-1/3 max-w-4xl min-w-max">
            {/* login section */}
            <div className="w-full p-5">
              <div className="text-center font-bold text-black">
                <span className="">Git</span>Core
              </div>
              <div className="py-10">
                <h2 className="text-3xl font-bold text-black mb-3">Sign in</h2>
                <div className="border-2 w-10 bg-sky-500 rounded-xl border-sky-500 inline-block mb-3"></div>
                {/* social media login */}
                <div className="flex justify-center my-3">
                  <a
                    href="http://"
                    className="border-2 border-gray-200 rounded-full p-3  mx-1"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <FaGithub className="text-sm" />
                  </a>
                  <a
                    href="http://"
                    className="border-2 border-gray-200 rounded-full p-3  mx-1"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <FaGoogle className="text-sm" />
                  </a>
                  <a
                    href="http://"
                    className="border-2 border-gray-200 rounded-full p-3  mx-1"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <FaLinkedinIn className="text-sm" />
                  </a>
                </div>
                <p className="text-gray-400 my-4">or use your email</p>
                <form action="">
                  {/* email container */}
                  <div className="flex flex-col items-center mb-3">
                    {/* <div
                    className="bg-gray-100  p-2 flex items-center rounded-xl  "
                  > */}
                    <input
                      type="email"
                      name="email"
                      id="email"
                      placeholder="Email"
                      className="m-1 rounded-xl border-none w-64 p-4 disabled  focus:outline-none focus:border-sky-500 focus:ring-1 focus:ring-sky-500
                      disabled:bg-slate-50 disabled:text-slate-500 disabled:border-slate-200 disabled:shadow-none
                      invalid:border-pink-500 invalid:text-red-600
                      focus:invalid:border-pink-500 focus:invalid:ring-red-500 border border-slate-300  text-sm shadow-sm placeholder-slate-400 bg-gray-100  outline-none flex-1 "
                    />
                    {/* </div> */}
                  </div>
                  {/* password container */}
                  <div className="flex flex-col items-center mb-3">
                    <input
                      type="password"
                      name="password"
                      id="password"
                      placeholder="Password"
                      className="m-1 rounded-xl border-none w-64 p-4 disabled  focus:outline-none focus:border-sky-500 focus:ring-1 focus:ring-sky-500
                      disabled:bg-slate-50 disabled:text-slate-500 disabled:border-slate-200 disabled:shadow-none
                      invalid:border-pink-500 invalid:text-red-600 mb-3
                      focus:invalid:border-pink-500 focus:invalid:ring-red-500 border border-slate-300  text-sm shadow-sm placeholder-slate-400 bg-gray-100  outline-none flex-1 "
                    />
                    <div className="flex justify-between w-64 mb-1 mt-2">
                      <label className="flex items center text-xs">
                        <input
                          type="checkbox"
                          name="remember"
                          id="remember"
                          className="mr-1"
                        />
                        Remember me
                      </label>
                      <a
                        className="text-xs"
                        href="http://"
                        target="_blank"
                        rel="noopener noreferrer"
                      >
                        Forgot Password?
                      </a>
                    </div>
                    <button className="font-semibold border-2 bg-sky-400 hover:border-sky-600 hover:bg-sky-200 group-hover: hover:transition rounded-xl w-64 p-3 m-4">
                      Sign in
                    </button>
                  </div>
                </form>
              </div>
            </div>

            {/* moto section */}
            {/* <div className="w-2/5 bg-green-500 rounded-tr-2xl rounded-br-2xl py-36 px-12">
            <h2 className="text-3xl font-bold mb-2">Developing Open source!</h2>
            <div className="border-2 w-10 border-white inline-block mb-2"></div>
            <p className="mb-2">
              Lorem ipsum dolor sit amet consectetur, adipisicing elit. Id et
              nihil quos, cum libero amet perspiciatis inventore fugiat
              repellendus eveniet sit? Corporis ratione quo, dolore quasi cumque
              nobis iure qui aut fugit?
            </p>
          </div> */}
          </div>
        </main>
      </div>
      ;
    </>
  );
}

export default Login;
