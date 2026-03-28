class Item {
    private int id;

    public Item(int id) {
        this.id = id;
    }

    @Override
    public int hashCode() {
        return id * 31;
    }
}

class HashCodeConsistency {
    public static void main(String[] args) {
        Item a = new Item(5);
        Item b = new Item(5);
        Item c = new Item(7);
        System.out.println(a.hashCode() == b.hashCode());
        System.out.println(a.hashCode() == c.hashCode());
    }
}
