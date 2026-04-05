class Box {
    public int value;
    Box(int v) {
        this.value = v;
    }
}

class PatternInstanceof {
    public static void main(String[] args) {
        Box b = new Box(42);
        if (b instanceof Box x) {
            System.out.println(x.value);
        }
        System.out.println("done");
    }
}
