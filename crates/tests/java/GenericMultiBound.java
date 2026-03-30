class Container<T extends Number & Comparable<T>> {
    private T item;

    public Container(T item) {
        this.item = item;
    }

    public T getItem() {
        return item;
    }
}

class GenericMultiBound {
    public static void main(String[] args) {
        Container<Integer> c = new Container<>(99);
        System.out.println(c.getItem());
    }
}
