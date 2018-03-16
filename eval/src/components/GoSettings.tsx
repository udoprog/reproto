import * as React from 'react';
import AceEditor from 'react-ace';

export interface GoSettings {
  encoding_json: boolean;
}

export interface GoSettingsFormProps {
  settings: GoSettings;
  onEncodingJson: (update: boolean) => void;
}

export class GoSettingsForm extends React.Component<GoSettingsFormProps, {}> {
  render() {
    let { settings } = this.props;

    return (
      <form>
      <div className="form-check mb-2">
        <input
          className="form-check-input"
          type="checkbox"
          checked={settings.encoding_json}
          onChange={e => this.props.onEncodingJson(e.target.checked)}
          id="go-json-net" />

        <label htmlFor="go-json-net" className="lb-sm">encoding/json Support</label>

        <small id="go-json-net-help" className="form-text form-text-sm text-muted">
          Support for <a href="https://golang.org/pkg/encoding/json/">encoding/json</a> tags
        </small>
      </div>
      </form>
    );
  }
}
