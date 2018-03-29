import * as React from "react";
import {Input} from "./Input";
import {OutputEditor} from "./OutputEditor";
import {JavaSettings, JavaSettingsForm} from "./JavaSettings";
import {CsharpSettings, CsharpSettingsForm} from "./CsharpSettings";
import {GoSettings, GoSettingsForm} from "./GoSettings";
import {RustSettings, RustSettingsForm} from "./RustSettings";
import {SwiftSettings, SwiftSettingsForm} from "./SwiftSettings";
import * as WebAssembly from "webassembly";
import {Annotation, Marker as AceMarker} from 'react-ace';
import AceEditor from 'react-ace';

const wasm = import("reproto_wasm");

const deepEqual = require("deep-equal");

const languages = [
  "csharp",
  "golang",
  "java",
  "javascript",
  "json",
  "python",
  "rust",
  "swift",
  "yaml",
]

const themes = [
  "monokai",
  "github",
]

const FORMAT_LANGUAGE_MAP: {[key: string]: string} = {
  csharp: "csharp",
  go: "golang",
  java: "java",
  js: "javascript",
  json: "json",
  python: "python",
  reproto: "reproto",
  rust: "rust",
  swift: "swift",
  yaml: "yaml",
};

// modes in local_modules
require("brace/mode/reproto.js")

languages.forEach((lang) => {
  require(`brace/mode/${lang}`)
  require(`brace/snippets/${lang}`)
})

themes.forEach((theme) => {
  require(`brace/theme/${theme}`)
})

// const DEFAULT_JSON = require("../static/default.json");
const DEFAULT_JSON = "{}";
const DEFAULT_YAML = require("../static/default.yaml");
const COMMON_REPROTO: string = require("../static/common.reproto");
const COMMON2_REPROTO: string = require("../static/common2.reproto");
const IMPORT_REPROTO: string = require("../static/import.reproto");
const TYPE_REPROTO: string = require("../static/type.reproto");
const TUPLE_REPROTO: string = require("../static/tuple.reproto");
const INTERFACE_REPROTO: string = require("../static/interface.reproto");
const DEFAULT_NEW_FILE_REPROTO: string = require("../static/default-new.reproto");
const logo = require("../static/logo.256.png");

interface Dialog {
  className: string;
  message: any;
}

interface Compiled {
  request: Derive;
  result: DeriveResult;
}

interface Derive {
  content: any;
  root_name: string;
  format: string;
  output: string;
  package_prefix: string;
}

interface Marker {
  message: string;
  row_start: number;
  row_end: number;
  col_start: number;
  col_end: number;
}

interface DeriveFile {
  path: string;
  content: string;
}

interface DeriveResult {
  files: DeriveFile[];
  error?: string;
  error_markers: Marker[];
  info_markers: Marker[];
}

enum Format {
  Json = "json",
  Yaml = "yaml",
  Reproto = "reproto",
}

enum Output {
  Reproto = "reproto",
  Java = "java",
  Csharp = "csharp",
  Go = "go",
  Swift = "swift",
  Rust = "rust",
  Python = "python",
  JavaScript = "js",
  Json = "json",
}

export interface MainProps {
}

interface ContentSet {
  [key: string]: string;
}

interface File {
  package: string;
  version?: string;
  content: string;
}

interface Settings {
  java: JavaSettings;
  csharp: CsharpSettings;
  go: GoSettings;
  rust: RustSettings;
  swift: SwiftSettings;
}

export interface MainState {
  contentSet: ContentSet;
  // set of files
  files: File[],
  // current selected package.
  file_index: number,
  // If we are editing the file metadata right now.
  file_editing_meta: boolean;
  // Settings for various outputs.
  settings: Settings;
  format: Format;
  output: Output;
  root_name: string;
  package_prefix: string;
  settings_enabled: boolean,
  // Error annotations (gutter markers) on input.
  input_annotations: Annotation[];
  // Error markers on input.
  input_markers: AceMarker[];
  // Result of last compilation.
  compiled?: Compiled;
  error?: string;
  derive?: (value: Derive) => DeriveResult;
}

export class Main extends React.Component<MainProps, MainState> {
  constructor(props: MainProps) {
    super(props);

    this.state = {
      contentSet: {
        json: DEFAULT_JSON,
        yaml: DEFAULT_YAML,
      },
      files: [
        {
          package: "example.type",
          content: TYPE_REPROTO,
        },
        {
          package: "example.interface",
          content: INTERFACE_REPROTO,
        },
        {
          package: "example.tuple",
          content: TUPLE_REPROTO,
        },
        {
          package: "example.import",
          content: IMPORT_REPROTO,
        },
        {
          package: "example.common",
          version: "1.0.0",
          content: COMMON_REPROTO,
        },
        {
          package: "example.common",
          version: "2.0.0",
          content: COMMON2_REPROTO,
        },
      ],
      file_index: 0,
      file_editing_meta: false,
      settings: {
        java: {
          jackson: true,
          lombok: true,
        },
        rust: {
          chrono: true,
        },
        csharp: {
          json_net: true,
        },
        go: {
          encoding_json: true,
        },
        swift: {
          codable: true,
          simple: false,
        },
      },
      root_name: "Generated",
      package_prefix: "reproto",
      settings_enabled: false,
      input_annotations: [],
      input_markers: [],
      format: Format.Reproto,
      output: Output.Java,
    };
  }

  componentWillUpdate(nextProps: MainProps, nextState: MainState) {
    // Update URL if needed
    const { format, output, file_index, files } = nextState;

    let f = files[file_index];

    let params = new URLSearchParams(location.search);
    let update = false;

    if (params.get("input") != format) {
      params.set("input", format);
      update = true;
    }

    if (params.get("output") != output) {
      params.set("output", output);
      update = true;
    }

    if (params.get("package") != f["package"]) {
      params.set("package", f["package"]);
      update = true;
    }

    if (update) {
      window.history.replaceState({}, "", location.pathname + "?" + params);
    }
  }

  componentDidMount() {
    const params = new URLSearchParams(location.search)

    const input = params.get("input")
    const output = params.get("output")
    const pkg = params.get("package")

    this.setPackage(pkg);
    this.setFormat(input);
    this.setOutput(output);

    wasm.then(mod => {
      this.setState({derive: mod.derive}, () => this.recompile());
    });
  }

  content(): string {
    let {format} = this.state;

    if (format == "reproto") {
      return this.state.files[this.state.file_index].content;
    } else {
      return this.state.contentSet[format];
    }
  }

  recompile() {
    this.setState((state: MainState, props: MainProps) => {
      let {
        contentSet,
        format,
        output,
        root_name,
        package_prefix,
        files,
        file_index,
        settings,
        compiled,
        derive,
      } = state;

      let content = this.content();

      if (!derive) {
        return {};
      }

      let compile = true;

      let content_request;

      if (format == "reproto") {
        content_request = {type: "file_index", index: file_index};
      } else {
        content_request = {type: "content", content: content};
      }

      const request = {
        content: content_request,
        files: files,
        root_name: root_name,
        package_prefix: package_prefix,
        format: format,
        output: output,
        settings: settings,
      };

      // no need to dispatch new request if it's identical to the old one...
      if (compiled && deepEqual(compiled.request, request)) {
        return {} as MainProps;
      }

      const result = derive(request) as DeriveResult;

      const input_annotations: Annotation[] = [];
      const input_markers: AceMarker[] = [];

      result.error_markers.forEach(m => {
        input_annotations.push({
          row: m.row_start,
          column: m.col_start,
          type: 'error',
          text: m.message,
        });

        input_markers.push({
          startRow: m.row_start,
          startCol: m.col_start,
          endRow: m.row_end,
          endCol: m.col_end,
          className: "error-marker",
          type: "background",
        });
      });

      result.info_markers.forEach(m => {
        input_annotations.push({
          row: m.row_start,
          column: m.col_start,
          type: 'info',
          text: m.message,
        });

        input_markers.push({
          startRow: m.row_start,
          startCol: m.col_start,
          endRow: m.row_end,
          endCol: m.col_end,
          className: "info-marker",
          type: "background",
        });
      });

      // Don't hide old result on errors.
      if (result.error && compiled) {
        result.files = compiled.result.files;
      }

      return {
        compiled: {
          request: request,
          result: result,
        },
        input_annotations: input_annotations,
        input_markers: input_markers,
      };
    });
  }

  setContent(format: Format, value: string) {
    console.log("new content", value.length);

    this.setState((state: MainState, props: MainProps) => {
      let {file_index, files, contentSet} = this.state;

      if (format == "reproto") {
        let new_files = files.map((file, index) => {
          if (index == file_index) {
            let new_file = {...file};
            new_file.content = value;
            return new_file;
          } else {
            return file;
          }
        });

        return {files: new_files} as MainState;
      } else {
        let new_content_set = {...contentSet};
        new_content_set[format] = value;
        return {contentSet: new_content_set} as MainState;
      }
    }, () => this.recompile());
  }

  setFile(file_index: number, cb: (file: File) => void) {
    this.setState((state: MainState, props: MainProps) => {
      let {files} = this.state;

      let new_files = files.map((file, index) => {
        if (index == file_index) {
          let new_file = {...file};
          cb(new_file);
          return new_file;
        } else {
          return file;
        }
      });

      return {files: new_files};
    }, () => this.recompile());
  }

  setFileIndex(value: string) {
    this.setState({
      file_index: Number(value)
    }, () => this.recompile());
  }

  setFormat(value: string) {
    let format;

    switch (value) {
      case "yaml":
        format = "yaml" as Format;
        break;
      case "reproto":
        format = "reproto" as Format;
        break;
      case "json":
        format = "json" as Format;
        break;
      default:
        return;
    }

    this.setState({
      format: format
    }, () => this.recompile());
  }

  setPackage(pkg: string) {
    this.setState((state: MainState, props: MainProps) => {
      const { files } = state;
      const index = files.findIndex((f: File) => f["package"] == pkg);

      if (index < 0) {
        return {} as MainState;
      }

      return {file_index: index};
    }, () => this.recompile());
  }

  setOutput(value: string) {
    let output;

    switch (value) {
      case "reproto":
        output = "reproto" as Output;
        break;
      case "java":
        output = "java" as Output;
        break;
      case "csharp":
        output = "csharp" as Output;
        break;
      case "go":
        output = "go" as Output;
        break;
      case "swift":
        output = "swift" as Output;
        break;
      case "python":
        output = "python" as Output;
        break;
      case "rust":
        output = "rust" as Output;
        break;
      case "js":
        output = "js" as Output;
        break;
      case "json":
        output = "json" as Output;
        break;
      default:
        return;
    }

    this.setState({
      output: output
    }, () => this.recompile());
  }

  setRootName(root_name: string) {
    this.setState({
      root_name: root_name
    }, () => this.recompile());
  }

  setPackagePrefix(package_prefix: string) {
    this.setState({
      package_prefix: package_prefix
    }, () => this.recompile());
  }

  updateJava(cb: (settings: JavaSettings) => void) {
    this.setState((state: MainState, props: MainProps) => {
      let settings = {...state.settings};
      settings.java = {...settings.java};
      cb(settings.java);
      return {settings: settings};
    }, () => this.recompile());
  }

  updateRust(cb: (settings: RustSettings) => void) {
    this.setState((state: MainState, props: MainProps) => {
      let settings = {...state.settings};
      settings.rust = {...settings.rust};
      cb(settings.rust);
      return {settings: settings};
    }, () => this.recompile());
  }

  updateSwift(cb: (settings: SwiftSettings) => void) {
    this.setState((state: MainState, props: MainProps) => {
      let settings = {...state.settings};
      settings.swift = {...settings.swift};
      cb(settings.swift);
      return {settings: settings};
    }, () => this.recompile());
  }

  updateCsharp(cb: (settings: CsharpSettings) => void) {
    this.setState((state: MainState, props: MainProps) => {
      let settings = {...state.settings};
      settings.csharp = {...settings.csharp};
      cb(settings.csharp);
      return {settings: settings};
    }, () => this.recompile());
  }

  updateGo(cb: (settings: GoSettings) => void) {
    this.setState((state: MainState, props: MainProps) => {
      let settings = {...state.settings};
      settings.go = {...settings.go};
      cb(settings.go);
      return {settings: settings};
    }, () => this.recompile());
  }

  newFile() {
    this.setState((state: MainState, props: MainProps) => {
      let { files } = state;

      files = [...files];
      let file_index = files.length;

      files.push({
        content: DEFAULT_NEW_FILE_REPROTO,
        package: "new"
      });

      return {
        files: files,
        file_index: file_index,
        file_editing_meta: true,
      };
    }, () => this.recompile());
  }

  deleteFile() {
    this.setState((state: MainState, props: MainProps) => {
      let { files, file_index } = state;

      return {
        files: files.filter((_, i: number) => i != file_index),
        file_index: 0,
        file_editing_meta: false,
      };
    }, () => this.recompile());
  }

  render() {
    let {
      contentSet,
      files,
      file_index,
      format,
      output,
      root_name,
      package_prefix,
      input_annotations,
      input_markers,
      settings,
      compiled,
      derive,
      settings_enabled,
    } = this.state;

    let content = this.content();

    let input_mode = FORMAT_LANGUAGE_MAP[format as string];
    let output_mode = FORMAT_LANGUAGE_MAP[output as string];

    let dialogs: Dialog[] = [];
    let compiledFiles: DeriveFile[] = [];

    var settingsForm = undefined;
    var view = undefined;

    if (format == "reproto") {
      let {version, package: file_package} = files[file_index];
      let {file_editing_meta} = this.state;

      if (file_editing_meta) {
        view = (
          <div className="form-row">
            <div className="input-group input-group-sm col mb-2">
              <div className="input-group-prepend">
                <label htmlFor="file-package" className="input-group-text lb-sm">File:</label>
              </div>

              <input
                id="file-package"
                type="text"
                className="form-control form-control-sm"
                placeholder="package"
                onChange={e => {
                  let value = e.target.value;
                  this.setFile(file_index, file => file.package = value);
                }}
                value={file_package} />
            </div>

            <div className="col-md-4 mb-2">
              <input
                id="file-version"
                type="text"
                className="form-control form-control-sm"
                placeholder="version"
                onChange={e => {
                  let value = e.target.value;
                  this.setFile(file_index, file => {
                    if (value == "") {
                      delete file.version;
                    } else {
                      file.version = value;
                    }
                  });
                }}
                value={version || ""} />
            </div>

            <div className="col-auto mb-2">
              <button
                type="button"
                title="Save file"
                className="btn btn-primary btn-sm w-100"
                onClick={() => {
                  this.setState({file_editing_meta: false});
                }}>
                <i className="fa fa-save"></i>
              </button>
            </div>

            <div className="col-auto mb-2">
              <button
                type="button"
                title="Delete file"
                className="btn btn-danger btn-sm w-100"
                onClick={() => {
                  this.deleteFile();
                }}>
                <i className="fa fa-trash"></i>
              </button>
            </div>
          </div>
        );
      } else {
        view = (
          <div className="form-row">
            <div className="input-group input-group-sm col mb-2">
              <div className="input-group-prepend">
                <label htmlFor="file-package" className="input-group-text lb-sm">File:</label>
              </div>

              <select
                id="file-package"
                value={file_index}
                className="form-control"
                onChange={e => this.setFileIndex(e.target.value)}>
                { files.map((f, index) => {
                  return <option key={index} value={index}>{f.package} {f.version || ""}</option>;
                }) }
              </select>
            </div>

            <div className="col-auto mb-2">
              <button
                type="button"
                title="Edit file name and version"
                className="btn btn-info btn-sm w-100"
                onClick={() => {
                  this.setState({file_editing_meta: true});
                }}>
                <i className="fa fa-edit"></i>
              </button>
            </div>

            <div className="col-auto mb-2">
              <button
                type="button"
                title="Add a new file"
                className="btn btn-success btn-sm w-100"
                onClick={() => {
                  this.newFile();
                }}>
                <i className="fa fa-plus"></i>
              </button>
            </div>

            <div className="col-auto mb-2">
              <button
                type="button"
                title="Delete file"
                className="btn btn-danger btn-sm w-100"
                onClick={() => {
                  this.deleteFile();
                }}>
                <i className="fa fa-trash"></i>
              </button>
            </div>
          </div>
        );
      }
    }

    if (!derive) {
      dialogs.push({
        className: "alert alert-info",
        message: (
          <div className="clearfix">
            <i className="fa fa-spinner fa-spin" style={{display: "inline-block", float: "left", fontSize: "150%"}} />
            <div className="ml-2" style={{display: "inline-block", float: "left"}}>
              <span>
                Loading <a href="https://github.com/reproto/reproto/blob/master/eval/reproto_wasm.rs" target="_blank">reproto wasm</a>
              </span>
              <br />
              <small>Did you know that wasm <a href="https://hacks.mozilla.org/2018/01/making-webassembly-even-faster-firefoxs-new-streaming-and-tiering-compiler/">loads <em>faster</em> in Firefox</a>?</small>
            </div>
          </div>
        ),
      });
    }

    if (format) {
      switch (output) {
        case "java":
          settingsForm = <JavaSettingsForm settings={settings.java}
            onJackson={update => this.updateJava(java => java.jackson = update)}
            onLombok={update => this.updateJava(java => java.lombok = update)}
            />;
          break;
        case "rust":
          settingsForm = <RustSettingsForm settings={settings.rust}
            onChrono={update => this.updateRust(rust => rust.chrono = update)}
            />;
          break;
        case "csharp":
          settingsForm = <CsharpSettingsForm settings={settings.csharp}
            onJsonNet={update => this.updateCsharp(csharp => csharp.json_net = update)}
            />;
          break;
        case "go":
          settingsForm = <GoSettingsForm settings={settings.go}
            onEncodingJson={update => this.updateGo(go => go.encoding_json = update)}
            />;
          break;
        case "swift":
          settingsForm = <SwiftSettingsForm settings={settings.swift}
            onCodable={update => this.updateSwift(swift => swift.codable = update)}
            onSimple={update => this.updateSwift(swift => swift.simple = update)}
            />;
          break;
        default:
          break;
      }
    }

    if (compiled) {
      let { error, error_markers, files } = compiled.result;

      if (files) {
        compiledFiles = files;
      }

      if (error) {
        if (error_markers.length == 0) {
          dialogs.push({
            className: "alert alert-danger",
            message: String(error),
          });
        }

        error_markers.forEach(m => {
          dialogs.push({
            className: "alert alert-danger",
            message: (
              <small>{m.row_start + 1}:{m.col_start}: {m.message}</small>
            ),
          });
        });
      }
    }

    return (
      <div className="box">
        <div className="box-row header">
          <nav className="navbar navbar-expand-lg navbar-light bg-light">
            <a className="navbar-brand" href="https://github.com/reproto">
              <img src={logo} width={32} height={32} title="reproto" />
            </a>
            <a className="navbar-brand mr-auto" href="#">reproto eval</a>

            <ul className="navbar-nav">
              <li className="nav-item">
                <a className="nav-link" href="https://github.com/reproto/reproto/tree/master/doc">
                  <i className="fa fa-book"></i>
                  &nbsp;
                  docs
                </a>
              </li>
              <li className="nav-item">
                <a className="nav-link" href="https://github.com/reproto/reproto">
                  <i className="fa fa-github"></i>
                  &nbsp;
                  reproto/reproto
                </a>
              </li>
            </ul>
          </nav>

          <div className="container-fluid">
            <div className="row mb-2 mt-2">
              <div className="col-6 col-xl-5 input">
                <form>
                  <div className="input-group input-group-sm mb-2">
                    <div className="input-group-prepend">
                      <label htmlFor="output" className="input-group-text lb-sm">Input:</label>
                    </div>

                    <select
                      id="format"
                      className="form-control"
                      value={format}
                      onChange={e => this.setFormat(e.target.value)}>
                      <option value="reproto">Reproto</option>
                      <option value="json">JSON (Derive)</option>
                      <option value="yaml">YAML (Derive)</option>
                    </select>
                  </div>

                  {view}
                </form>
              </div>

              <div className="col">
                <form>
                  <div className="form-row">
                    <div className="input-group input-group-sm col mb-2">
                      <div className="input-group-prepend">
                        <label htmlFor="output" className="input-group-text lb-sm">Output:</label>
                      </div>

                      <select
                        id="output"
                        className="form-control"
                        value={output}
                        onChange={e => this.setOutput(e.target.value)}>
                        <option value="csharp">C#</option>
                        <option value="go">Go</option>
                        <option value="java">Java</option>
                        <option value="js">JavaScript</option>
                        <option value="json">JSON (RpIR)</option>
                        <option value="python">Python</option>
                        <option value="reproto">Reproto</option>
                        <option value="rust">Rust</option>
                        <option value="swift">Swift</option>
                      </select>
                    </div>
                  </div>

                  <div className="form-row">
                    <div className="input-group input-group-sm col-md-4 mb-2">
                      <div className="input-group-prepend">
                        <label htmlFor="package-prefix" className="input-group-text lb-sm">Package:</label>
                      </div>

                      <input
                        id="package-prefix"
                        type="text"
                        className="form-control form-control-sm"
                        value={package_prefix}
                        onChange={e => this.setPackagePrefix(e.target.value)} />
                    </div>

                    {format != "reproto" ?
                    <div className="input-group input-group-sm col-md-4 mb-2">
                      <div className="input-group-prepend">
                        <label htmlFor="root-name" className="input-group-text lb-sm">Generated Name:</label>
                      </div>

                      <input
                        id="root-name"
                        type="text"
                        className="form-control form-control-sm"
                        value={root_name}
                        onChange={e => this.setRootName(e.target.value)} />
                    </div> : undefined}

                    <div className="input-group-sm col-auto mb-2">
                      <button className="btn btn-sm btn-light"
                        type="button"
                        title="Show Settings"
                        style={{display: settings_enabled ? 'none' : null }}
                        disabled={!settingsForm}
                        onClick={() => {
                          this.setState({settings_enabled: true});
                        }}>
                        <i className="fa fa-cog"></i>
                        &nbsp;
                        Show
                      </button>
                      <button className="btn btn-sm btn-dark"
                        type="button"
                        title="Hide Settings"
                        style={{display: settings_enabled ? null : 'none' }}
                        disabled={!settingsForm}
                        onClick={() => {
                          this.setState({settings_enabled: false});
                        }}>
                        <i className="fa fa-cog"></i>
                        &nbsp;
                        Hide
                      </button>
                    </div>
                  </div>
                </form>

                {settings_enabled ? settingsForm : undefined}
              </div>
            </div>
          </div>
        </div>

        <div className="box-row content container-fluid">
          <div className="row editors">
            <div className="col-6 col-xl-5 input">
              <AceEditor
                tabSize={2}
                showGutter={true}
                mode={input_mode}
                theme="monokai"
                width="100%"
                height="100%"
                value={content}
                annotations={input_annotations}
                markers={input_markers}
                onChange={value => this.setContent(format, value)}
                />
            </div>

            <div className="col output">
              {dialogs.length > 0 ? (
                <div className="dialogs row mt-2">
                  <div className="col">
                    {dialogs.map((d, key) => (
                      <div key={key} className={d.className}>
                        {d.message}
                      </div>
                    ))}
                  </div>
                </div>
              ) : undefined}

              {compiledFiles.map((f, index) => {
                return (
                  <div key={index} className="output-file">
                    <div className="title">
                      <i className="title-icon fa fa-file"></i>
                      <span className="title-text">{f.path}</span>
                    </div>
                    <OutputEditor mode={output_mode as string} value={f.content} />
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    );
  }
}
