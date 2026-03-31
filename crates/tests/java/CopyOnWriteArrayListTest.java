import java.util.concurrent.CopyOnWriteArrayList;

class CopyOnWriteArrayListTest {
    public static void main(String[] args) {
        CopyOnWriteArrayList<String> list = new CopyOnWriteArrayList<>();

        list.add("a");
        list.add("b");
        list.add("c");
        System.out.println("size = " + list.size());
        System.out.println("get(1) = " + list.get(1));

        // contains
        System.out.println("contains b = " + list.contains("b"));
        System.out.println("contains z = " + list.contains("z"));

        // indexOf
        System.out.println("indexOf c = " + list.indexOf("c"));
        System.out.println("indexOf z = " + list.indexOf("z"));

        // set
        String old = list.set(1, "B");
        System.out.println("replaced = " + old);
        System.out.println("get(1) after set = " + list.get(1));

        // remove
        String removed = list.remove(0);
        System.out.println("removed = " + removed);
        System.out.println("size after remove = " + list.size());

        // isEmpty
        System.out.println("isEmpty = " + list.isEmpty());

        // clear
        list.clear();
        System.out.println("isEmpty after clear = " + list.isEmpty());
    }
}
