package test;

import io.reproto.OkHttpSerialization;
import java.io.IOException;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import okhttp3.Call;
import okhttp3.Callback;
import okhttp3.HttpUrl;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;

public interface MyService {
  public class OkHttp implements MyService {
    private final OkHttpClient client;
    private final HttpUrl baseUrl;
    private final OkHttpSerialization serialization;

    public OkHttp(
      final OkHttpClient client,
      final HttpUrl baseUrl,
      final OkHttpSerialization serialization
    ) {
      this.client = client;
      this.baseUrl = baseUrl;
      this.serialization = serialization;
    }

    public CompletableFuture<Void> unknown(final int id) {
      final HttpUrl url_ = this.baseUrl.newBuilder()
        .addPathSegment("unknown")
        .addPathSegment(Integer.toString(id))
        .build();

      final Request req_ = new Request.Builder()
        .url(url_)
        .method("GET", null)
        .build();

      final CompletableFuture<Void> future_ = new CompletableFuture<Void>();

      this.client.newCall(req_).enqueue(new Callback() {
        @Override
        public void onFailure(final Call call, final IOException e) {
          future_.completeExceptionally(e);
        }

        @Override
        public void onResponse(final Call call, final Response response) {
          if (!response.isSuccessful()) {
            future_.completeExceptionally(new IOException("bad response: " + response));
          } else {
            future_.complete(null);
          }
        }
      });

      return future_;
    }

    public CompletableFuture<Entry> unknownReturn(final int id) {
      final HttpUrl url_ = this.baseUrl.newBuilder()
        .addPathSegment("unknown-return")
        .addPathSegment(Integer.toString(id))
        .build();

      final Request req_ = new Request.Builder()
        .url(url_)
        .method("GET", null)
        .build();

      final CompletableFuture<Entry> future_ = new CompletableFuture<Entry>();

      this.client.newCall(req_).enqueue(new Callback() {
        @Override
        public void onFailure(final Call call, final IOException e) {
          future_.completeExceptionally(e);
        }

        @Override
        public void onResponse(final Call call, final Response response) {
          if (!response.isSuccessful()) {
            future_.completeExceptionally(new IOException("bad response: " + response));
          } else {
            future_.complete(OkHttp.this.serialization.decode(response.body(), Entry.class));
          }
        }
      });

      return future_;
    }

    public CompletableFuture<Void> unknownArgument(final Entry request, final int id) {
      final HttpUrl url_ = this.baseUrl.newBuilder()
        .addPathSegment("unknown-argument")
        .addPathSegment(Integer.toString(id))
        .build();

      final Request req_ = new Request.Builder()
        .url(url_)
        .method("GET", OkHttp.this.serialization.encode(request))
        .build();

      final CompletableFuture<Void> future_ = new CompletableFuture<Void>();

      this.client.newCall(req_).enqueue(new Callback() {
        @Override
        public void onFailure(final Call call, final IOException e) {
          future_.completeExceptionally(e);
        }

        @Override
        public void onResponse(final Call call, final Response response) {
          if (!response.isSuccessful()) {
            future_.completeExceptionally(new IOException("bad response: " + response));
          } else {
            future_.complete(null);
          }
        }
      });

      return future_;
    }

    public CompletableFuture<Entry> unary(final Entry request, final int id) {
      final HttpUrl url_ = this.baseUrl.newBuilder()
        .addPathSegment("unary")
        .addPathSegment(Integer.toString(id))
        .build();

      final Request req_ = new Request.Builder()
        .url(url_)
        .method("GET", OkHttp.this.serialization.encode(request))
        .build();

      final CompletableFuture<Entry> future_ = new CompletableFuture<Entry>();

      this.client.newCall(req_).enqueue(new Callback() {
        @Override
        public void onFailure(final Call call, final IOException e) {
          future_.completeExceptionally(e);
        }

        @Override
        public void onResponse(final Call call, final Response response) {
          if (!response.isSuccessful()) {
            future_.completeExceptionally(new IOException("bad response: " + response));
          } else {
            future_.complete(OkHttp.this.serialization.decode(response.body(), Entry.class));
          }
        }
      });

      return future_;
    }
  }

  public static class OkHttpBuilder {
    private Optional<HttpUrl> baseUrl = Optional.empty();
    private Optional<OkHttpSerialization> serialization = Optional.empty();
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

    public OkHttpBuilder serialization(final OkHttpSerialization serialization) {
      this.serialization = Optional.of(serialization);
      return this;
    }

    public OkHttp build() {
      final HttpUrl baseUrl = this.baseUrl.orElseThrow(() -> new RuntimeException("baseUrl: is a required field"));
      final OkHttpSerialization serialization = this.serialization.orElseThrow(() -> new RuntimeException("serialization: is a required field"));
      return new OkHttp(client, baseUrl, serialization);
    }
  }
}
