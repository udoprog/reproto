package test;

import java.util.Optional;
import okhttp3.HttpUrl;
import okhttp3.OkHttpClient;

public interface MyService {
  public class OkHttp implements MyService {
    private final OkHttpClient client;
    private final HttpUrl baseUrl;

    public OkHttp(
      final OkHttpClient client,
      final HttpUrl baseUrl
    ) {
      this.client = client;
      this.baseUrl = baseUrl;
    }

    public void unknown() {
      throw new RuntimeException("endpoint does not support HTTP");
    }

    public Entry unknownReturn() {
      throw new RuntimeException("endpoint does not support HTTP");
    }

    public void unknownArgument(final Entry request) {
      throw new RuntimeException("endpoint does not support HTTP");
    }

    public Entry unary(final Entry request) {
      throw new RuntimeException("endpoint does not support HTTP");
    }

    public Entry serverStreaming(final Entry request) {
      throw new RuntimeException("endpoint does not support HTTP");
    }

    public Entry clientStreaming(final Entry request) {
      throw new RuntimeException("endpoint does not support HTTP");
    }

    public Entry bidiStreaming(final Entry request) {
      throw new RuntimeException("endpoint does not support HTTP");
    }
  }

  public static class OkHttpBuilder {
    private Optional<HttpUrl> baseUrl = Optional.empty();
    private final OkHttpClient client;

    public OkHttpBuilder(
      final OkHttpClient client
    ) {
      this.client = client;
    }

    public OkHttpBuilder baseUrl(final HttpUrl baseUrl) {
      this.baseUrl = Optional.of(baseUrl);
      return this;
    }

    public OkHttp build() {
      final HttpUrl baseUrl = this.baseUrl.orElseThrow(() -> new RuntimeException("baseUrl: is a required field"));
      return new OkHttp(client, baseUrl);
    }
  }
}
