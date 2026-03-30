class BoundedBox<T extends Comparable<T>> {
    private T value;

    public BoundedBox(T v) {
        this.value = v;
    }

    public T get() {
        return value;
    }
}

class GenericBounded {
    public static void main(String[] args) {
        BoundedBox<Integer> b1 = new BoundedBox<>(42);
        System.out.println(b1.get());

        BoundedBox<String> b2 = new BoundedBox<>("bounded");
        System.out.println(b2.get());
    }
}
