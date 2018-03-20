package test;

import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import okhttp3.Call;
import okhttp3.Callback;
import okhttp3.HttpUrl;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;

public interface MyService {
  /**
   * <pre>
   * UNKNOWN
   * </pre>
   */
  CompletableFuture<Void> unknown(final int id);

  /**
   * <pre>
   * UNKNOWN
   * </pre>
   */
  CompletableFuture<Entry> unknownReturn(final int id);

  /**
   * <pre>
   * UNKNOWN
   * </pre>
   */
  CompletableFuture<Void> unknownArgument(final Entry request, final int id);

  /**
   * <pre>
   * UNARY
   * </pre>
   */
  CompletableFuture<Entry> unary(final Entry request, final int id);

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

    @Override
    public CompletableFuture<Void> unknown(final int id) {
      final HttpUrl url = this.baseUrl.newBuilder()
        .addPathSegment("unknown")
        .addPathSegment(Integer.toString(id))
        .build();
      final Request request = new Request.Builder()
        .url(url)
        .build();
      client.newCall(request).enqueue(new Callback{
        @Override
        public void onFailure(final Call call, final Call e) {
          future.fail(e);
        }

        @Override
        public void onResponse(final Call call, final Response response) {
          if (!response.isSuccessful()) {
            future.fail(new IOException("bad response: " + response))
            return;
          }
        }
      });
      throw new IllegalStateException("not implemented");
    }

    @Override
    public CompletableFuture<Entry> unknownReturn(final int id) {
      final HttpUrl url = this.baseUrl.newBuilder()
        .addPathSegment("unknown-return")
        .addPathSegment(Integer.toString(id))
        .build();
      final Request request = new Request.Builder()
        .url(url)
        .build();
      client.newCall(request).enqueue(new Callback{
        @Override
        public void onFailure(final Call call, final Call e) {
          future.fail(e);
        }

        @Override
        public void onResponse(final Call call, final Response response) {
          if (!response.isSuccessful()) {
            future.fail(new IOException("bad response: " + response))
            return;
          }
        }
      });
      throw new IllegalStateException("not implemented");
    }

    @Override
    public CompletableFuture<Void> unknownArgument(final Entry request, final int id) {
      final HttpUrl url = this.baseUrl.newBuilder()
        .addPathSegment("unknown-argument")
        .addPathSegment(Integer.toString(id))
        .build();
      final Request request = new Request.Builder()
        .url(url)
        .build();
      client.newCall(request).enqueue(new Callback{
        @Override
        public void onFailure(final Call call, final Call e) {
          future.fail(e);
        }

        @Override
        public void onResponse(final Call call, final Response response) {
          if (!response.isSuccessful()) {
            future.fail(new IOException("bad response: " + response))
            return;
          }
        }
      });
      throw new IllegalStateException("not implemented");
    }

    @Override
    public CompletableFuture<Entry> unary(final Entry request, final int id) {
      final HttpUrl url = this.baseUrl.newBuilder()
        .addPathSegment("foo")
        .addPathSegment(Integer.toString(id))
        .build();
      final Request request = new Request.Builder()
        .url(url)
        .build();
      client.newCall(request).enqueue(new Callback{
        @Override
        public void onFailure(final Call call, final Call e) {
          future.fail(e);
        }

        @Override
        public void onResponse(final Call call, final Response response) {
          if (!response.isSuccessful()) {
            future.fail(new IOException("bad response: " + response))
            return;
          }
        }
      });
      throw new IllegalStateException("not implemented");
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
