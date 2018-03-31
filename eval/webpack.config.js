const path = require("path");

const HtmlWebpackPlugin = require("html-webpack-plugin");
const CopyWebpackPlugin = require("copy-webpack-plugin");

module.exports = {
  mode: "development",

  entry: [
    "./src/index.tsx",
    "./src/main.scss",
  ],

  output: {
    filename: "bundle.js",
    path: __dirname + "/dist"
  },

  devtool: "source-map",

  resolve: {
    extensions: [".ts", ".tsx", ".js"],
    modules: ["node_modules", "local_modules"],
    alias: {
      "rust": path.resolve("./target/wasm32-unknown-unknown/release/")
    }
  },

  module: {
    rules: [
      {
        test: /\.tsx?$/,
        use: [
          {
            loader: "babel-loader",
            options: { presets: ['@babel/preset-env'] }
          },
          "awesome-typescript-loader",
        ],
      },
      {
        test: /\.js?$/,
        loader: "babel-loader",
        exclude: /(node_modules|bower_components)/,
        options: { presets: ['@babel/preset-env'] }
      },
      {
        test: /\.json$/,
        loader: "raw-loader",
      },
      { enforce: "pre", test: /\.js$/, loader: "source-map-loader" },
      { test: /\-wasm\.js$/, loader: "exports-loader?__initialize" },
      { test: /\.scss$/, loader: "css-loader!sass-loader" },
      { test: /\.(jpe?g|gif|png)$/, loader: "file-loader" },
      { test: /.(ttf|otf|eot|svg|woff(2)?)(\?[a-z0-9]+)?$/,
        use: [{
          loader: 'file-loader',
          options: {
            name: '[name].[ext]',
            outputPath: 'fonts/',    // where the fonts will go
            publicPath: '/fonts/'       // override the default path
          }
        }]
      }
    ]
  },

  /// External react components permitting them to be loaded through CDN.
  externals: {
    "webassembly": "WebAssembly",
  },

  plugins: [
    new HtmlWebpackPlugin({
      template: "index.html"
    }),
    new CopyWebpackPlugin([
      "target/wasm32-unknown-unknown/release/reproto-wasm.wasm",
      "src/static/favicon.ico",
    ])
  ],
};
