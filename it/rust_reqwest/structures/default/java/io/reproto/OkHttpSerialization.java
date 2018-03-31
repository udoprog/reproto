package io.reproto;

import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.IOException;
import okhttp3.MediaType;
import okhttp3.RequestBody;
import okhttp3.ResponseBody;

public interface OkHttpSerialization {
  /**
   * Encode the request body.
   */
  <T> RequestBody encode(final T entity);

  /**
   * Decode the response body.
   */
  <T> T decode(final ResponseBody body, final Class<T> cls);

  public class Jackson implements OkHttpSerialization {
    private static final MediaType JSON = MediaType.parse("application/json; charset=utf-8");
    private final ObjectMapper m;

    public Jackson(
      final ObjectMapper m
    ) {
      this.m = m;
    }

    /**
     * Decode the response body.
     */
    @Override
    public <T> T decode(final ResponseBody body, final Class<T> cls) {
      try {
        return m.readValue(body.bytes(), cls);
      } catch (final IOException e) {
        throw new RuntimeException(e);
      }
    }

    /**
     * Encode the request body.
     */
    @Override
    public <T> RequestBody encode(final T entity) {
      final byte[] buffer;

      try {
        buffer = m.writeValueAsBytes(entity);
      } catch (final IOException e) {
        throw new RuntimeException(e);
      }

      return RequestBody.create(JSON, buffer);
    }
  }
}
