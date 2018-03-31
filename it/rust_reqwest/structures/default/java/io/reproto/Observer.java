package io.reproto;


public interface Observer<V> {
  public void onCompleted();

  public void onError(final Throwable error);

  public void onNext(final V value);
}
